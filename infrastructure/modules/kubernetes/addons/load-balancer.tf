# infrastructure/modules/kubernetes/addons/load-balancer.tf

resource "helm_release" "aws_lb_controller" {
  name       = "aws-load-balancer-controller"
  repository = "https://aws.github.io/eks-charts"
  chart      = "aws-load-balancer-controller"
  namespace  = "kube-system"

  wait = true # On attend qu'il soit prêt pour ne pas bloquer les futurs Webhooks

  set = [
    { name = "clusterName", value = var.cluster_name },
    { name = "serviceAccount.name", value = "aws-load-balancer-controller" },
    { name = "serviceAccount.annotations.eks\\.amazonaws\\.com/role-arn", value = var.lb_controller_role_arn },
    { name = "nodeSelector.intent", value = "system" }
  ]

  values = [yamlencode({
    tolerations = [{
      key = "CriticalAddonsOnly", operator = "Equal", value = "true", effect = "NoSchedule"
    }]
  })]
}