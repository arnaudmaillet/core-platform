output "vpc_id" {
  value = aws_vpc.main.id
}

output "public_subnet_ids" {
  value = aws_subnet.public[*].id
}

output "private_app_subnet_ids" {
  value = aws_subnet.private_apps[*].id
}

output "private_data_subnet_ids" {
  value = aws_subnet.private_data[*].id
}

output "vpc_cidr_block" {
  value = aws_vpc.main.cidr_block
}