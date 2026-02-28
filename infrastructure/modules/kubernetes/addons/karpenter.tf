# infrastructure/modules/kubernetes/addons/karpenter.tf

resource "helm_release" "karpenter" {
  name                = "karpenter"
  repository          = "oci://public.ecr.aws/karpenter"
  chart               = "karpenter"
  version             = "1.2.1"
  namespace           = "kube-system"
  create_namespace    = true

  values = [
    yamlencode({
      settings = {
        clusterName       = var.cluster_name
        interruptionQueue = ""
      }
      serviceAccount = {
        create = true
        name   = "karpenter"
        annotations = {
          "eks.amazonaws.com/role-arn" = var.karpenter_controller_role_arn
        }
      }
      controller = {
        clusterEndpoint = var.cluster_endpoint
        nodeSelector = { intent = "system" }
        tolerations = [{
          key      = "CriticalAddonsOnly"
          operator = "Equal"
          value    = "true"
          effect   = "NoSchedule"
        }]
      }
    })
  ]
}