import grpc from 'k6/net/grpc';
import { check, sleep } from 'k6';

const client = new grpc.Client();

// Chargement du proto (6 niveaux pour remonter à la racine du monorepo)
client.load(['../../../../../../proto'], 'profile/v1/profile.proto');

export const options = {
    stages: [
        { duration: '30s', target: 20 },  // Montée en charge progressive
        { duration: '1m', target: 100 }, // Pic à 100 utilisateurs
        { duration: '30s', target: 0 },   // Redescente
    ],
    thresholds: {
        'grpc_req_duration': ['p(95)<2000'], // On accepte 2s au p95 à cause de la contention SQL
    },
};

// Fonction utilitaire pour générer un UUID v4 (si tu as plusieurs profils en DB)
function uuidv4() {
    return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function(c) {
        let r = Math.random() * 16 | 0, v = c === 'x' ? r : (r & 0x3 | 0x8);
        return v.toString(16);
    });
}

export default () => {
    // Connexion au serveur via le tunnel kubectl
    client.connect('localhost:50051', { plaintext: true });

    const payload = {
        // Option 1: Ton ID fixe qui existe déjà en DB
        profile_id: "550e8400-e29b-41d4-a716-446655440000",

        // Option 2: Décommente si tu as seedé plusieurs IDs aléatoires
        // profile_id: uuidv4(),

        new_handle: `handle_${Math.floor(Math.random() * 1000000)}`,
    };

    const params = {
        metadata: { 'x-region': 'eu' }
    };

    const response = client.invoke('profile.v1.ProfileIdentityService/UpdateHandle', payload, params);

    // Débogage (optionnel, à commenter pendant le vrai stress test)
    if (__ITER === 0) {
        console.log(`Debug - Status: ${response.status} (Type: ${typeof response.status})`);
    }

    check(response, {
        // On vérifie le code 0 (OK) en format nombre ou objet
        'status is OK': (r) => r && (r.status === 0 || r.status === grpc.StatusOk),
    });

    client.close();

    // Petite pause pour laisser respirer le pool de connexion Postgres
    sleep(0.1);
};