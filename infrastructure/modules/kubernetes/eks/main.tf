# infrastructure/modules/kubernetes/eks/main.tf

# --- MODULE EKS PRINCIPAL ---
module "eks" {
  source  = "terraform-aws-modules/eks/aws"
  version = "~> 20.0"

  cluster_name    = var.cluster_name
  cluster_version = "1.31"

  cluster_endpoint_public_access = true

  vpc_id     = var.vpc_id
  subnet_ids = var.private_subnet_ids

  enable_cluster_creator_admin_permissions = true
  enable_irsa                              = true

  # Groupes de machines (Managed Node Groups)
  eks_managed_node_groups = {

    system = {
      instance_types = var.system_node_settings.instance_types
      min_size       = var.system_node_settings.min_size
      max_size       = var.system_node_settings.max_size
      desired_size   = var.system_node_settings.desired_size
      labels         = { "intent" = "system" }
    }

    management = {
      instance_types = var.mgmt_node_settings.instance_types
      min_size       = var.mgmt_node_settings.min_size
      max_size       = var.mgmt_node_settings.max_size
      desired_size   = var.mgmt_node_settings.desired_size
      labels         = { "intent" = "management" }
    }

    database = {
      instance_types = var.db_node_settings.instance_types
      min_size       = var.db_node_settings.min_size
      max_size       = var.db_node_settings.max_size
      desired_size   = var.db_node_settings.desired_size

      taints = [{
        key    = "dedicated"
        value  = "database"
        effect = "NO_SCHEDULE"
      }]

      iam_role_additional_policies = {
        AmazonEBSCSIDriverPolicy = "arn:aws:iam::aws:policy/service-role/AmazonEBSCSIDriverPolicy"
      }
      labels = { "role" = "storage" }
    }
  }

  node_security_group_tags = {
    "karpenter.sh/discovery" = var.cluster_name
  }
}



# # ---------------------------------------------------------------------------------------------------------------------
# # MODULE EKS PRINCIPAL : Provisionne le cluster Kubernetes managé sur AWS
# # ---------------------------------------------------------------------------------------------------------------------
# module "eks" {
#   source  = "terraform-aws-modules/eks/aws" # Utilise le module communautaire standard pour AWS
#   version = "~> 20.0"                      # Version majeure 20 pour bénéficier des dernières fonctionnalités (Access Entries)
#
#   cluster_name    = var.cluster_name       # Nom unique du cluster (ex: "prod-eks-cluster")
#   cluster_version = "1.31"                 # Version stable de Kubernetes (K8s)
#
#   # Autorise l'accès à l'API server via l'Internet (sécurisé par IAM/RBAC)
#   # Utile pour pouvoir gérer le cluster depuis ton poste ou une CI/CD externe
#   cluster_endpoint_public_access = true
#
#   vpc_id     = var.vpc_id                  # ID du réseau virtuel où déployer le cluster
#   subnet_ids = var.private_subnet_ids      # Place les nœuds dans des sous-réseaux privés pour la sécurité
#
#   # Donne automatiquement les droits "ClusterAdmin" à l'utilisateur IAM qui lance ce Terraform
#   enable_cluster_creator_admin_permissions = true
#
#   # Active l'IAM Roles for Service Accounts (IRSA) pour lier des rôles AWS à des pods K8s
#   enable_irsa = true
#
#   # --- Groupes de machines (EC2) managés par AWS ---
#   eks_managed_node_groups = {
#
#     # GROUPE 'SYSTEM' : Pour les outils d'infrastructure (DNS, Metrics, Autoscaler)
#     system = {
#       instance_types = var.eks_instance_types_system # Ex: t3.medium (économique)
#       min_size       = var.eks_min_size             # Taille minimale du groupe
#       max_size       = var.eks_max_size             # Taille maximale (en cas de pic)
#       desired_size   = var.eks_desired_size            # Nombre de machines au démarrage
#
#       labels = { "intent" = "control-plane" }       # Label K8s pour cibler ce groupe
#     }
#
#     # GROUPE 'DATABASE' : Dédié à Postgres (CNPG) et ScyllaDB
#     database = {
#       instance_types = var.eks_instance_types_database # Ex: r6i.large (optimisé RAM/IO)
#       min_size       = var.eks_min_size
#       max_size       = var.eks_max_size
#       desired_size   = var.eks_desired_size
#
#       # TAINT (Anti-Affinité) : Empêche les pods "normaux" (APIs, Web) de se mettre ici.
#       # Seuls les pods avec une "toleration" spécifique peuvent s'installer sur ces nœuds.
#       taints = [{
#         key    = "dedicated"
#         value  = "database"
#         effect = "NO_SCHEDULE"
#       }]
#
#       # Ajoute la politique IAM EBS aux nœuds pour qu'ils puissent attacher/détacher des volumes
#       iam_role_additional_policies = {
#         AmazonEBSCSIDriverPolicy = "arn:aws:iam::aws:policy/service-role/AmazonEBSCSIDriverPolicy"
#       }
#
#       labels = { "role" = "storage" } # Label utile pour les NodeSelectors des bases de données
#     }
#   }
#
#   # Tag indispensable pour que Karpenter (l'autoscaler) sache quels sous-réseaux il peut utiliser
#   node_security_group_tags = {
#     "karpenter.sh/discovery" = var.cluster_name
#   }
# }
#
# # ---------------------------------------------------------------------------------------------------------------------
# # DNS & SSL : Gestion du nom de domaine et du certificat pour l'accès gRPC
# # ---------------------------------------------------------------------------------------------------------------------
#
# # 1. On récupère la zone Route 53 que tu as créée pour le domaine
# data "aws_route53_zone" "main" {
#   name         = "core-platform.click"
#   private_zone = false
# }
#
# # 2. On crée le certificat SSL (ACM) pour le domaine et ses sous-domaines
# resource "aws_acm_certificate" "cert" {
#   domain_name       = "core-platform.click"
#   validation_method = "DNS"
#
#   # On ajoute un wildcard pour couvrir api.core-platform.click, dev.core-platform.click, etc.
#   subject_alternative_names = ["*.core-platform.click"]
#
#   lifecycle {
#     create_before_destroy = true
#   }
# }
#
# # 3. Création automatique du record DNS pour valider que tu es le proprio du domaine
# resource "aws_route53_record" "cert_validation" {
#   for_each = {
#     for dvo in aws_acm_certificate.cert.domain_validation_options : dvo.domain_name => {
#       name   = dvo.resource_record_name
#       record = dvo.resource_record_value
#       type   = dvo.resource_record_type
#     }
#   }
#
#   allow_overwrite = true
#   name            = each.value.name
#   records         = [each.value.record]
#   ttl             = 60
#   type            = each.value.type
#   zone_id         = data.aws_route53_zone.main.zone_id
# }
#
# # 4. Cette ressource "attend" que la validation DNS soit terminée côté AWS
# resource "aws_acm_certificate_validation" "cert" {
#   certificate_arn         = aws_acm_certificate.cert.arn
#   validation_record_fqdns = [for record in aws_route53_record.cert_validation : record.fqdn]
# }
#
# # On expose l'ARN pour pouvoir le copier dans l'Ingress Kubernetes
# output "ssl_certificate_arn" {
#   value = aws_acm_certificate.cert.arn
# }
#
# # ---------------------------------------------------------------------------------------------------------------------
# # IAM POUR LOAD BALANCER CONTROLLER : Permet à K8s de piloter les ALB/NLB AWS
# # ---------------------------------------------------------------------------------------------------------------------
# resource "aws_iam_policy" "load_balancer_controller" {
#   name   = "${var.cluster_name}-lb-controller-policy"
#   policy = var.iam_policy_json_content
# }
#
#
# module "lb_controller_irsa_role" {
#   source  = "terraform-aws-modules/iam/aws//modules/iam-role-for-service-accounts-eks"
#   version = "~> 5.0"
#
#   role_name = "${var.cluster_name}-lb-controller-role"
#
#   # On utilise l'ARN de la politique qu'on vient de définir juste au-dessus
#   role_policy_arns = {
#     policy = aws_iam_policy.load_balancer_controller.arn
#   }
#
#   oidc_providers = {
#     main = {
#       provider_arn               = module.eks.oidc_provider_arn
#       namespace_service_accounts = ["kube-system:aws-load-balancer-controller"]
#     }
#   }
# }
#
# # Rôle IAM pour External-DNS (permet au cluster de mettre à jour Route53 tout seul)
# module "external_dns_irsa_role" {
#   source  = "terraform-aws-modules/iam/aws//modules/iam-role-for-service-accounts-eks"
#   version = "~> 5.0"
#
#   role_name                     = "${var.cluster_name}-external-dns-role"
#   attach_external_dns_policy    = true # Le module inclut déjà la politique standard
#   external_dns_hosted_zone_arns = [data.aws_route53_zone.main.arn]
#
#   oidc_providers = {
#     main = {
#       provider_arn               = module.eks.oidc_provider_arn
#       namespace_service_accounts = ["kube-system:external-dns"]
#     }
#   }
# }
#
# # ---------------------------------------------------------------------------------------------------------------------
# # IAM POUR EBS : Permet à K8s de créer des disques durs AWS (EBS) dynamiquement
# # ---------------------------------------------------------------------------------------------------------------------
# module "ebs_csi_irsa_role" {
#   source  = "terraform-aws-modules/iam/aws//modules/iam-role-for-service-accounts-eks"
#   version = "~> 5.0"
#
#   role_name             = "${var.cluster_name}-ebs-csi-role" # Nom du rôle IAM
#   attach_ebs_csi_policy = true                             # Attache la politique de gestion des disques EBS
#
#   oidc_providers = {
#     main = {
#       provider_arn               = module.eks.oidc_provider_arn           # Lien de confiance avec le cluster EKS
#       namespace_service_accounts = ["kube-system:ebs-csi-controller-sa"] # Seul ce compte K8s peut utiliser ce rôle
#     }
#   }
# }
#
# # ---------------------------------------------------------------------------------------------------------------------
# # KARPENTER : Autoscaler intelligent (remplace le Cluster Autoscaler classique)
# # ---------------------------------------------------------------------------------------------------------------------
# module "karpenter" {
#   source  = "terraform-aws-modules/eks/aws//modules/karpenter"
#   version = "~> 20.0"
#
#   cluster_name = module.eks.cluster_name
#
#   # Lie le rôle IAM des nœuds à Karpenter pour qu'il puisse créer de nouvelles EC2
#   node_iam_role_name    = module.eks.node_iam_role_name
#   enable_v1_permissions = true # Active les permissions pour la version v1 (dernière version)
# }
#
# # ---------------------------------------------------------------------------------------------------------------------
# # STORAGE CLASS : Définit le type de disque par défaut pour les bases de données
# # ---------------------------------------------------------------------------------------------------------------------
# resource "kubernetes_storage_class_v1" "gp3" {
#   metadata {
#     name = "gp3" # Nom utilisé dans les PersistentVolumeClaims (PVC)
#     annotations = {
#       # Rend ce stockage automatique si l'utilisateur ne précise rien
#       "storageclass.kubernetes.io/is-default-class" = "true"
#     }
#   }
#   storage_provisioner    = "ebs.csi.aws.com"         # Utilise le driver AWS EBS
#   reclaim_policy         = "Delete"                  # Supprime le disque AWS si le PVC K8s est supprimé
#   allow_volume_expansion = true                      # Permet d'augmenter la taille du disque à chaud
#   volume_binding_mode    = "WaitForFirstConsumer"    # N'attache le disque que lorsqu'on sait où est le Pod
#   parameters = {
#     type      = "gp3"                                # Nouvelle génération de disques AWS (plus performant/moins cher)
#     encrypted = "true"                               # Chiffrement des données au repos
#   }
#
#   depends_on = [module.eks] # Attendre que le cluster soit prêt avant de créer la classe
# }
#
# # ---------------------------------------------------------------------------------------------------------------------
# # OPERATORS & TOOLS : Installation via Helm (Gestionnaire de paquets K8s)
# # ---------------------------------------------------------------------------------------------------------------------
#
# # 1. CERT-MANAGER : Gère les certificats SSL/TLS internes (indispensable pour les Webhooks Scylla/Postgres)
# resource "helm_release" "cert_manager" {
#   name             = "cert-manager"
#   repository       = "https://charts.jetstack.io"
#   chart            = "cert-manager"
#   version          = "v1.13.0"
#   namespace        = "cert-manager"
#   create_namespace = true # Crée le namespace s'il n'existe pas
#
#   set = [{ name  = "installCRDs", value = "true" }] # Installe les définitions de ressources personnalisées
#   depends_on = [module.eks]
# }
#
# # Installation du Driver EBS via Helm
# resource "helm_release" "aws_ebs_csi_driver" {
#   name       = "aws-ebs-csi-driver"
#   repository = "https://kubernetes-sigs.github.io/aws-ebs-csi-driver"
#   chart      = "aws-ebs-csi-driver"
#   namespace  = "kube-system"
#
#   set = [
#     {
#       name  = "controller.serviceAccount.annotations.eks\\.amazonaws\\.com/role-arn"
#       value = module.ebs_csi_irsa_role.iam_role_arn
#     }
#   ]
#
#   depends_on = [module.eks]
# }
#
# # 2. CLOUDNATIVE-PG : L'Operator qui gère les clusters PostgreSQL (Haute Disponibilité, Backups)
# resource "helm_release" "cnpg" {
#   name             = "cloudnative-pg"
#   repository       = "https://cloudnative-pg.github.io/charts"
#   chart            = "cloudnative-pg"
#   namespace        = "cnpg-system"
#   create_namespace = true
#
#   depends_on = [module.eks]
# }
#
# # 3. SCYLLADB OPERATOR : Gère la base NoSQL ScyllaDB (Installation de l'intelligence de gestion)
# resource "helm_release" "scylla_operator" {
#   name             = "scylla-operator"
#   repository       = "https://scylla-operator-charts.storage.googleapis.com/stable"
#   chart            = "scylla-operator"
#   namespace        = "scylla-operator"
#   create_namespace = true
#
#   # L'operator Scylla a besoin de Cert-Manager pour valider ses configurations
#   depends_on = [helm_release.cert_manager]
# }
#
# # Installation de l'AWS Load Balancer Controller
# resource "helm_release" "aws_lb_controller" {
#   name       = "aws-load-balancer-controller"
#   repository = "https://aws.github.io/eks-charts"
#   chart      = "aws-load-balancer-controller"
#   namespace  = "kube-system" # Toujours dans kube-system
#
#   set = [
#     {
#       name  = "clusterName"
#       value = module.eks.cluster_name
#     },
#     {
#       name  = "serviceAccount.create"
#       value = "true"
#     },
#     {
#       name  = "serviceAccount.name"
#       value = "aws-load-balancer-controller"
#     },
#     {
#       name  = "serviceAccount.annotations.eks\\.amazonaws\\.com/role-arn"
#       value = module.lb_controller_irsa_role.iam_role_arn
#     },
#     {
#       name  = "vpcId"
#       value = var.vpc_id
#     }
#   ]
#
#   depends_on = [module.eks]
# }
#
# # ---------------------------------------------------------------------------------------------------------------------
# # PROVIDERS : Version finale stabilisée
# # ---------------------------------------------------------------------------------------------------------------------
#
# provider "kubernetes" {
#   host                   = module.eks.cluster_endpoint
#   cluster_ca_certificate = base64decode(module.eks.cluster_certificate_authority_data)
#
#   exec {
#     api_version = "client.authentication.k8s.io/v1beta1"
#     command     = "aws"
#     args        = ["eks", "get-token", "--cluster-name", module.eks.cluster_name]
#   }
# }
#
# provider "helm" {
#   kubernetes = {
#     host                   = module.eks.cluster_endpoint
#     cluster_ca_certificate = base64decode(module.eks.cluster_certificate_authority_data)
#
#     # On utilise "exec" comme une clé de la map
#     exec = {
#       api_version = "client.authentication.k8s.io/v1beta1"
#       command     = "aws"
#       args        = ["eks", "get-token", "--cluster-name", module.eks.cluster_name]
#     }
#   }
# }