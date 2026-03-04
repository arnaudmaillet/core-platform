# infrastructure/modules/kubernetes/addons/external-dns.tf

resource "helm_release" "external_dns" {
  name       = "external-dns"
  repository = "https://kubernetes-sigs.github.io/external-dns/"
  chart      = "external-dns"
  namespace  = "kube-system"

  values = [
    yamlencode({
      provider = "aws"
      policy   = "sync"
      sources = ["service", "ingress"] 
      txtOwnerId = var.cluster_name

      aws = {
        region = var.aws_region
        preferCNAME = false
      }
      
      serviceAccount = {
        create = true
        name   = "external-dns"
        annotations = {
          "eks.amazonaws.com/role-arn" = var.external_dns_role_arn
        }
      }
      domainFilters = ["core-platform.click"]
    })
  ]
}