#!/bin/bash
# backend/services/profile/api/command-server/tests/init-stress-test.sh

set -e

DB_POD=$(kubectl get pod -l "cnpg.io/cluster=dev-profile-db,role=primary" -o name)
NAMESPACE="default"
OUTPUT_FILE="./data.json"

echo "🚀 Initialisation des données sur $DB_POD..."

# 1. On vide si besoin (optionnel) et on injecte
kubectl exec $DB_POD -n $NAMESPACE -- psql -U postgres -d profile -c "
TRUNCATE user_profiles CASCADE;
INSERT INTO user_profiles (id, owner_id, region_code, display_name, handle)
SELECT gen_random_uuid(), gen_random_uuid(), 'eu', 'User_' || i, 'handle_' || i || '_' || (random()*1000)::int
FROM generate_series(1, 1000) s(i);"

echo "📥 Extraction des IDs vers $OUTPUT_FILE..."

# 2. Extraction propre
kubectl exec $DB_POD -n $NAMESPACE -- psql -U postgres -d profile -t -A -c "SELECT id FROM user_profiles LIMIT 500;" \
| sed '/^$/d' | jq -R . | jq -s . > $OUTPUT_FILE

echo "✅ Prêt ! Tu peux lancer : k6 run test.js"