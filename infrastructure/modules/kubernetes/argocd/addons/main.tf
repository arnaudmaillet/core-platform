# infrastructure/modules/kubernetes/argocd/addons/main.tf

resource "aws_eks_addon" "this" {
  for_each = var.addons

  cluster_name                = var.cluster_name
  addon_name                  = each.key
  addon_version               = lookup(each.value, "addon_version", null) 
  service_account_role_arn    = each.value.service_account_role_arn
  resolve_conflicts_on_create = "OVERWRITE"
  resolve_conflicts_on_update = "OVERWRITE"
}