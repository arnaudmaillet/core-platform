# docs/architecture/external_systems.dsl

fcm = softwareSystem "FCM / APNS" "Push Notification Gateways (Google/Apple)." "External"
emailProvider = softwareSystem "Email Provider" "Service d'envoi d'emails transactionnels (SendGrid/Postmark)." "External"
objectStorage = softwareSystem "S3 / MinIO" "Stockage d'objets immuables (Images/Vidéos)." "Infrastructure"
cdn = softwareSystem "CDN" "Edge Content Delivery Network pour le caching des médias." "Infrastructure"
keycloak = softwareSystem "Keycloak (IAM)" "Gestionnaire d'identité, OAuth2/OIDC et SSO." "Infrastructure"
aiService = softwareSystem "AI Moderation API" "Analyse automatisée (AWS Rekognition / Google Vision)." "External"