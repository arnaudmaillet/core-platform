# infrastructure/live/dev/us-east-1/kubernetes/addons/apps/terragrunt.hcl

include "root" { path = find_in_parent_folders("root.hcl") }

dependency "core"    { config_path = "../core" }
dependency "eks"     { config_path = "../../eks" }
dependency "route53" { config_path = "../../../networking/route53" }

terraform {
  source = "../../../../../../modules/kubernetes/addons/apps"

  # --- NETTOYAGE GLOBAL (APRÈS TERRAFORM) ---
#   after_hook "cleanup_aws_resources" {
#     commands     = ["destroy"]
#     run_on_error = true
#     # ASTUCE : On appelle directement la dependency ici, SANS passer par un local
#     execute      = ["/bin/bash", "-c", <<-EOT
#       aws eks update-kubeconfig --region us-east-1 --name ${dependency.eks.outputs.cluster_name}
      
#       echo "🔍 Nettoyage des instances EC2 Karpenter orphelines..."
#       INSTANCE_IDS=$(aws ec2 describe-instances \
#         --filters "Name=tag:kubernetes.io/cluster/${dependency.eks.outputs.cluster_name},Values=owned" "Name=instance-state-name,Values=running" \
#         --query "Reservations[*].Instances[?!(Tags[?Key=='eks:nodegroup-name'])].InstanceId" \
#         --output text)

#       if [ -n "$INSTANCE_IDS" ] && [ "$INSTANCE_IDS" != "None" ]; then
#         echo "⚡️ Termination forcée : $INSTANCE_IDS"
#         aws ec2 terminate-instances --instance-ids $INSTANCE_IDS
#         aws ec2 wait instance-terminated --instance-ids $INSTANCE_IDS
#       fi

#       echo "🧹 Nettoyage des Ingress Finalizers..."
#       INGRESS_LIST=$(kubectl get ingress -A -o jsonpath='{range .items[*]}{.metadata.namespace}{"/"}{.metadata.name}{" "}{end}' 2>/dev/null)
#       for ns_name in $INGRESS_LIST; do
#         NS=$(echo $ns_name | cut -d'/' -f1); NAME=$(echo $ns_name | cut -d'/' -f2)
#         kubectl patch ingress $NAME -n $NS -p '{"metadata":{"finalizers":null}}' --type=merge || true
#         kubectl delete ingress $NAME -n $NS --force --grace-period=0 || true
#       done
#     EOT
#     ]
#   }

#   after_hook "fix_k6_state_on_failure" {
#     commands     = ["destroy"]
#     execute      = ["terragrunt", "state", "rm", "helm_release.k6_operator"]
#     run_on_error = true
#   }
}

inputs = {
  # On repasse aux appels directs ici, c'est supporté dans le bloc inputs
  cluster_name           = dependency.eks.outputs.cluster_name
  cluster_endpoint       = dependency.eks.outputs.cluster_endpoint
  cluster_ca_certificate = dependency.eks.outputs.cluster_certificate_authority_data
  
  ssl_certificate_arn           = dependency.eks.outputs.ssl_certificate_arn
  route53_zone_id               = dependency.route53.outputs.zone_id
  external_dns_role_arn         = dependency.eks.outputs.external_dns_role_arn
  karpenter_controller_role_arn = dependency.eks.outputs.karpenter_controller_role_arn

  enable_core_addons    = false
  enable_apps_addons    = true
}