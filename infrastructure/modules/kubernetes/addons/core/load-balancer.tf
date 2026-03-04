# infrastructure/modules/kubernetes/addons/core/load-balancer.tf

resource "helm_release" "aws_lb_controller" {
  name       = "aws-load-balancer-controller"
  repository = "https://aws.github.io/eks-charts"
  chart      = "aws-load-balancer-controller"
  namespace  = "kube-system"

  values = [yamlencode({
    clusterName = var.cluster_name
    serviceAccount = {
      name = "aws-load-balancer-controller"
      annotations = {
        "eks.amazonaws.com/role-arn" = var.lb_controller_role_arn
      }
    }
    nodeSelector = {
      intent = "system"
    }
    tolerations = [{
      key      = "CriticalAddonsOnly"
      operator = "Equal"
      value    = "true"
      effect   = "NoSchedule"
    }]
  })]
}