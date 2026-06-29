# infrastructure/modules/networking/vpc/main.tf

# --- VPC PRINCIPAL ---
resource "aws_vpc" "main" {
  cidr_block           = var.vpc_cidr
  enable_dns_hostnames = true
  enable_dns_support   = true

  tags = {
    Name = "${var.project_name}-${var.env}-vpc"
  }
}

# --- INTERNET GATEWAY ---
resource "aws_internet_gateway" "main" {
  vpc_id = aws_vpc.main.id
  tags   = { Name = "${var.project_name}-${var.env}-igw" }
}

# --- SUBNETS PUBLICS ---
# Utilisés pour les Load Balancers et NAT Gateways
resource "aws_subnet" "public" {
  count  = length(var.availability_zones)
  vpc_id = aws_vpc.main.id
  # Plage : 10.0.0.0/24, 10.0.1.0/24, 10.0.2.0/24
  cidr_block              = cidrsubnet(var.vpc_cidr, 8, count.index)
  availability_zone       = var.availability_zones[count.index]
  map_public_ip_on_launch = true

  tags = {
    Name                                        = "${var.project_name}-public-${var.availability_zones[count.index]}"
    "kubernetes.io/role/elb"                    = "1"
    "kubernetes.io/cluster/${var.cluster_name}" = "shared"
  }
}

# --- SUBNETS PRIVÉS APPS (EKS) ---
# GRANDS subnets pour supporter des milliers de Pods
resource "aws_subnet" "private_apps" {
  count  = length(var.availability_zones)
  vpc_id = aws_vpc.main.id
  # Plage : 10.0.16.0/20, 10.0.32.0/20, 10.0.48.0/20
  # On commence à l'index 1 (saut de 16) pour éviter le conflit avec les publics
  cidr_block        = cidrsubnet(var.vpc_cidr, 4, count.index + 1)
  availability_zone = var.availability_zones[count.index]

  tags = {
    Name                                        = "${var.project_name}-private-apps-${var.availability_zones[count.index]}"
    "kubernetes.io/role/internal-elb"           = "1"
    "kubernetes.io/cluster/${var.cluster_name}" = "shared"
    "karpenter.sh/discovery"                    = var.cluster_name
  }
}

# --- SUBNETS PRIVÉS DATA (Bases de données) ---
# Isolés et petits
resource "aws_subnet" "private_data" {
  count  = length(var.availability_zones)
  vpc_id = aws_vpc.main.id
  # Plage : 10.0.200.0/24, 10.0.201.0/24, etc.
  cidr_block        = cidrsubnet(var.vpc_cidr, 8, count.index + 200)
  availability_zone = var.availability_zones[count.index]

  tags = {
    Name = "${var.project_name}-private-data-${var.availability_zones[count.index]}"
    Tier = "data"
  }
}

# --- NAT GATEWAYS (egress for the private app subnets) ---
# single_nat_gateway = true  -> one shared NAT (cheapest; fine for dev/staging).
# single_nat_gateway = false -> one NAT per AZ (HA): an AZ outage no longer kills
# fleet-wide egress, and each AZ egresses through its LOCAL NAT, so pod->NAT
# traffic stays in-AZ (no cross-AZ data-transfer charges). The env.hcl flag is
# now actually honored — previously it was dead config and NAT was always single.
locals {
  nat_count = var.single_nat_gateway ? 1 : length(var.availability_zones)
}

resource "aws_eip" "nat" {
  count  = local.nat_count
  domain = "vpc"
  tags   = { Name = "${var.project_name}-${var.env}-nat-${count.index}" }
}

resource "aws_nat_gateway" "main" {
  count         = local.nat_count
  allocation_id = aws_eip.nat[count.index].id
  subnet_id     = aws_subnet.public[count.index].id
  tags          = { Name = "${var.project_name}-${var.env}-nat-${count.index}" }
  depends_on    = [aws_internet_gateway.main]
}

# Preserve the previously-unindexed NAT on already-applied (single-NAT) envs.
moved {
  from = aws_nat_gateway.main
  to   = aws_nat_gateway.main[0]
}

# --- ROUTAGE ---
resource "aws_route_table" "public" {
  vpc_id = aws_vpc.main.id
  route {
    cidr_block = "0.0.0.0/0"
    gateway_id = aws_internet_gateway.main.id
  }
}

resource "aws_route_table_association" "public" {
  count          = length(var.availability_zones)
  subnet_id      = aws_subnet.public[count.index].id
  route_table_id = aws_route_table.public.id
}

# One private route table per NAT; each private-app subnet routes to the NAT in
# its AZ (or all share the single NAT when single_nat_gateway = true).
resource "aws_route_table" "private" {
  count  = local.nat_count
  vpc_id = aws_vpc.main.id
  route {
    cidr_block     = "0.0.0.0/0"
    nat_gateway_id = aws_nat_gateway.main[count.index].id
  }
  tags = { Name = "${var.project_name}-${var.env}-rt-private-${count.index}" }
}

moved {
  from = aws_route_table.private
  to   = aws_route_table.private[0]
}

resource "aws_route_table_association" "private_apps" {
  count          = length(var.availability_zones)
  subnet_id      = aws_subnet.private_apps[count.index].id
  route_table_id = aws_route_table.private[var.single_nat_gateway ? 0 : count.index].id
}

resource "aws_route_table" "data" {
  vpc_id = aws_vpc.main.id
  tags   = { Name = "${var.project_name}-rt-data" }
}

resource "aws_route_table_association" "private_data" {
  count          = length(var.availability_zones)
  subnet_id      = aws_subnet.private_data[count.index].id
  route_table_id = aws_route_table.data.id
}

# --- VPC ENDPOINTS (keep AWS-service traffic off the NAT) ---------------------
# S3 Gateway endpoint is FREE and removes all S3 traffic from the NAT data-
# processing meter — significant for the media (presigned objects), audit (WORM
# checkpoints) and ECR-layer (image pulls) paths. Associated with the private-app
# and data route tables; routing through the endpoint instead of the NAT.
resource "aws_vpc_endpoint" "s3" {
  vpc_id            = aws_vpc.main.id
  service_name      = "com.amazonaws.${var.aws_region}.s3"
  vpc_endpoint_type = "Gateway"
  route_table_ids   = concat(aws_route_table.private[*].id, [aws_route_table.data.id])
  tags              = { Name = "${var.project_name}-${var.env}-s3-gw" }
}

# Interface endpoints for the AWS APIs the fleet hits on the hot path (ECR pulls,
# STS for IRSA, Secrets Manager for ESO, KMS for audit/MSK). Off by default
# (each endpoint bills per-AZ-hour); enable in staging/prod where the NAT data-
# processing they offload exceeds their cost. dev keeps NAT-only.
resource "aws_security_group" "vpc_endpoints" {
  count       = var.enable_interface_endpoints ? 1 : 0
  name_prefix = "${var.project_name}-${var.env}-vpce-"
  description = "HTTPS from the VPC to interface endpoints"
  vpc_id      = aws_vpc.main.id

  ingress {
    description = "HTTPS from the VPC"
    from_port   = 443
    to_port     = 443
    protocol    = "tcp"
    cidr_blocks = [var.vpc_cidr]
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = { Name = "${var.project_name}-${var.env}-vpce" }

  lifecycle {
    create_before_destroy = true
  }
}

resource "aws_vpc_endpoint" "interface" {
  for_each = var.enable_interface_endpoints ? toset(var.interface_endpoint_services) : []

  vpc_id              = aws_vpc.main.id
  service_name        = "com.amazonaws.${var.aws_region}.${each.value}"
  vpc_endpoint_type   = "Interface"
  subnet_ids          = aws_subnet.private_apps[*].id
  security_group_ids  = [aws_security_group.vpc_endpoints[0].id]
  private_dns_enabled = true
  tags                = { Name = "${var.project_name}-${var.env}-vpce-${replace(each.value, ".", "-")}" }
}