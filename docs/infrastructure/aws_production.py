import os
from diagrams import Cluster, Diagram, Edge
from diagrams.aws.compute import EKS, EC2
from diagrams.aws.network import Route53, ALB, CloudFront, GlobalAccelerator
from diagrams.aws.storage import S3
from diagrams.aws.security import ACM
from diagrams.onprem.database import PostgreSQL, Scylla, Clickhouse
from diagrams.onprem.network import Nginx
from diagrams.onprem.search import Solr
from diagrams.onprem.queue import Kafka
from diagrams.onprem.monitoring import Prometheus
from diagrams.onprem.gitops import Argocd
from diagrams.onprem.inmemory import Redis
from diagrams.custom import Custom

keycloak_icon_path = "docs/icons/keycloak_icon.png"

graph_attr = {
    "fontsize": "30",
    "bgcolor": "white",
    "splines": "ortho",
    "nodesep": "2.0",
    "ranksep": "2.0"
}

BASE_DIR = os.getcwd() 
keycloak_icon_path = os.path.join(BASE_DIR, "docs/icons/keycloak_icon.png")

with Diagram(
    "Social Network Global Architecture", 
    show=False, 
    direction="TB", 
    filename="docs/infrastructure/aws_production_diagram",
    graph_attr=graph_attr
):
    # --- GLOBAL ---
    dns = Route53("Global DNS")
    cdn = CloudFront("Edge CDN / WAF")
    g_acc = GlobalAccelerator("Traffic Manager")

    with Cluster("Region A (Primary)"):
        ingress = ALB("Ingress Gateway")
        s3_data = S3("Object Storage")

        # --- AZ-1 (ACTIVE) ---         
        with Cluster("EKS Cluster (Auto-scaling & Topology Aware)"):
            # SYSTEM
            with Cluster("NodePool: System & Ops"):
                system = [
                    Argocd("ArgoCD"), 
                    EC2("Karpenter"), 
                    Prometheus("Metrics"),
                    ACM("Cert-Manager"),
                    Nginx("External-DNS")
                ]

            # CORE SERVICES (Stateless)
            with Cluster("NodePool: Core Services"):
                with Cluster("BFF Layer"):
                    api_bff = EKS("API BFF")
                    live_bff = EKS("Live BFF")

                with Cluster("Internal Services (Private)", graph_attr={"style":"dashed"}):
                    row1 = [EKS("Account"), EKS("Profile"), EKS("Social Graph")]
                    row2 = [EKS("Post"), EKS("Comment"), EKS("Engagement")]
                    row3 = [EKS("Feed"), EKS("Notif"), EKS("Media Svc")]

            # WORKERS (Définit ici pour corriger l'erreur)
            with Cluster("NodePool: Workers"):
                workers = [EKS("Search Wkr"), EKS("Media Wkr"), EKS("Data Wkr")]

            # STORAGE (Stateful & Dedicated)
            with Cluster("NodePool: Storage"):
                pg_cluster = PostgreSQL("Postgres")
                scylla_cluster = Scylla("ScyllaDB")
                keycloak = Custom("Keycloak IAM", keycloak_icon_path)
                kafka_bus = Kafka("Kafka")
                redis_session = Redis("Redis (Sessions)")
                redis_data = Redis("Redis (Services)")
                
                olap_db = Clickhouse("ClickHouse")
                search_idx = Solr("OpenSearch")

    # --- FAILOVER ---
    with Cluster("Region B (Failover)"):
        dr_pg = PostgreSQL("PG Replica")
        dr_scylla = Scylla("Scylla Replica")
        dr_s3 = S3("Storage Replica")

    # --- FLUX ---
    user_f = Edge(color="royalblue")
    data_f = Edge(color="forestgreen", style="dashed")
    async_f = Edge(color="darkorange", style="bold")
    sync_f = Edge(color="firebrick", style="dotted")
    cdn_f = Edge(color="purple", style="bold")

    ingress >> Edge(color="royalblue") >> [api_bff, live_bff]

    # 1. Entrée & CDN
    dns >> cdn
    cdn >> g_acc >> ingress
    cdn >> cdn_f >> s3_data
    
    # 2. Authentification & BFF
    ingress >> user_f >> [api_bff, live_bff]
    [api_bff, live_bff] >> user_f >> keycloak
    keycloak >> data_f >> pg_cluster
    [api_bff, live_bff] >> data_f >> redis_session
    
    # 3. API Internes
    api_bff >> user_f >> (row1 + row2 + row3)
    
    # 4. Persistence
    row1[0] >> data_f >> pg_cluster
    (row1[1:] + row2) >> data_f >> scylla_cluster
    row3[0] >> data_f >> redis_data
    row3[2] >> data_f >> s3_data
    
    # 5. Async pipeline
    (row2 + row3) >> async_f >> kafka_bus
    kafka_bus >> async_f >> workers
    workers[0] >> data_f >> search_idx
    workers[1] >> data_f >> s3_data
    workers[2] >> data_f >> olap_db

    # 6. Replication
    pg_cluster >> sync_f >> dr_pg
    scylla_cluster >> sync_f >> dr_scylla
    s3_data >> sync_f >> dr_s3