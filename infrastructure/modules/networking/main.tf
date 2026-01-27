# infrastructure/modules/networking/main.tf

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
  count                   = length(var.availability_zones)
  vpc_id                  = aws_vpc.main.id
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
  count             = length(var.availability_zones)
  vpc_id            = aws_vpc.main.id
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
  count             = length(var.availability_zones)
  vpc_id            = aws_vpc.main.id
  # Plage : 10.0.200.0/24, 10.0.201.0/24, etc.
  cidr_block        = cidrsubnet(var.vpc_cidr, 8, count.index + 200)
  availability_zone = var.availability_zones[count.index]

  tags = {
    Name = "${var.project_name}-private-data-${var.availability_zones[count.index]}"
    Tier = "data"
  }
}

# --- NAT GATEWAY (Sortie Internet pour les Apps) ---
resource "aws_eip" "nat" {
  count = 1 # Pour la DEV on en met un seul, en PROD on en mettrait un par AZ
  domain = "vpc"
}

resource "aws_nat_gateway" "main" {
  allocation_id = aws_eip.nat[0].id
  subnet_id     = aws_subnet.public[0].id
  tags          = { Name = "${var.project_name}-nat" }
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

resource "aws_route_table" "private" {
  vpc_id = aws_vpc.main.id
  route {
    cidr_block     = "0.0.0.0/0"
    nat_gateway_id = aws_nat_gateway.main.id
  }
}

resource "aws_route_table_association" "private_apps" {
  count          = length(var.availability_zones)
  subnet_id      = aws_subnet.private_apps[count.index].id
  route_table_id = aws_route_table.private.id
}