#!/bin/bash
# backend/services/profile/api/command-server/load-tests/seed-test-data.sh

set -e

DB_POD=$(kubectl get pod -l "cnpg.io/cluster=dev-profile-db,role=primary" -o name)
NAMESPACE="default"
DATA_FILE="./data.json"

echo "🧪 Lecture des IDs depuis $DATA_FILE..."
# On transforme le JSON en une liste SQL (UUID, UUID, ...)
IDS=$(jq -r '.[]' $DATA_FILE | sed "s/.*/('&', gen_random_uuid(), 'eu', 'LoadTestUser', 'handle_test')/" | paste -sd "," -)

echo "🚀 Injection dans la DB..."
kubectl exec -i $DB_POD -n $NAMESPACE -- psql -U postgres -d profile -c "
DELETE FROM user_profiles WHERE display_name = 'LoadTestUser';
INSERT INTO user_profiles (id, owner_id, region_code, display_name, handle)
VALUES $IDS;"

echo "✅ DB synchronisée avec data.json !"