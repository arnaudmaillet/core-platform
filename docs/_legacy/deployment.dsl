# docs/deployment.dsl

deploymentEnvironment "Production" {

        deploymentNode "Amazon Web Services" "Cloud Provider" "AWS" {
            region = deploymentNode "eu-west-3" "Paris" {

                # --- LAYER 1: EDGE & NETWORK ---
                route53 = deploymentNode "Route 53" "DNS" "AWS Route 53"
                cdn = deploymentNode "CloudFront" "Content Delivery" "AWS CloudFront"
                waf = deploymentNode "WAF" "Firewall" "AWS WAF"

                vpc = deploymentNode "VPC" "Réseau privé virtuel" "10.0.0.0/16" {
                    
                    # --- LAYER 2: PUBLIC SUBNETS (Load Balancers) ---
                    deploymentNode "Public Subnet AZ1" "Zone A" "10.0.1.0/24" {
                        alb = deploymentNode "Application Load Balancer" "Répartiteur de charge" "AWS ALB"
                    }

                    # --- LAYER 3: PRIVATE SUBNETS (Microservices / EKS) ---
                    deploymentNode "Private Subnet App" "Compute Cluster" {
                        eks = deploymentNode "EKS Cluster" "Kubernetes" "Amazon EKS" {
                            
                            deploymentNode "API Node Group" "Stateless" "m5.large (Auto-scaling)" {
                                apiInstance = containerInstance apiBff
                            }
                            
                            deploymentNode "Live Node Group" "Stateful (WebSockets)" "c5.xlarge" {
                                liveInstance = containerInstance liveBff
                            }

                            deploymentNode "Services Node Group" "Business Logic" "t3.medium" {
                                containerInstance postService
                                containerInstance commentService
                                containerInstance engagementService
                                # ... tous les autres services Rust ...
                            }
                        }
                    }

                    # --- LAYER 4: DATA SUBNETS (Managed & High Perf) ---
                    deploymentNode "Private Subnet Data" "Persistence Layer" {
                        
                        msk = deploymentNode "Amazon MSK" "Kafka Managé" "3 Nodes (Multi-AZ)" {
                            containerInstance kafka
                        }

                        elasticache = deploymentNode "ElastiCache" "Redis Cluster" "Primary + Replicas" {
                            containerInstance redisBff
                            containerInstance redisEngagement
                            containerInstance redisPubSub
                        }

                        scyllaCluster = deploymentNode "ScyllaDB Cloud" "NoSQL Cluster" "i3.en instances (NVMe)" {
                            containerInstance postDb
                            containerInstance commentDb
                            containerInstance graphDb
                        }
                    }
                }
            }
        }

        # --- RELATIONS DE DÉPLOIEMENT ---
        route53 -> cdn "Résolution DNS"
        cdn -> alb "Forwarding du trafic dynamique"
        alb -> apiInstance "HTTP API Traffic (Port 80/443)"
        alb -> liveInstance "WebSocket Traffic (WSS)"
    }