#[cfg(test)]
mod tests {
    use crate::core::{Error, Result};
    use crate::messaging::{EventEnvelope, EventProducer, OutboxProcessor, OutboxStore};
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::sync::watch;
    use uuid::Uuid;

    // --- MOCK STRUCTURES ---

    #[derive(Clone)]
    struct MockStoreState {
        unprocessed: Arc<Mutex<Vec<EventEnvelope>>>,
        processed_ids: Arc<Mutex<Vec<Uuid>>>,
        fetch_calls: Arc<AtomicUsize>,
        should_fail: Arc<Mutex<bool>>,
    }

    struct MockStore {
        state: MockStoreState,
    }

    #[async_trait]
    impl OutboxStore for MockStore {
        async fn fetch_unprocessed(&self, limit: u32) -> Result<Vec<EventEnvelope>> {
            self.state.fetch_calls.fetch_add(1, Ordering::SeqCst);

            if *self.state.should_fail.lock().unwrap() {
                return Err(Error::database("Simulated database failure"));
            }

            let mut unprocessed = self.state.unprocessed.lock().unwrap();
            let len = unprocessed.len().min(limit as usize);
            let drained: Vec<EventEnvelope> = unprocessed.drain(0..len).collect();
            Ok(drained)
        }

        async fn mark_as_processed(&self, ids: &[Uuid]) -> Result<()> {
            let mut processed = self.state.processed_ids.lock().unwrap();
            processed.extend_from_slice(ids);
            Ok(())
        }

        async fn mark_as_failed(&self, _id: Uuid, _last_error: String) -> Result<()> {
            Ok(())
        }
    }

    #[derive(Clone)]
    struct MockBrokerState {
        published: Arc<Mutex<Vec<EventEnvelope>>>,
        should_fail: Arc<Mutex<bool>>,
    }

    struct MockBroker {
        state: MockBrokerState,
    }

    #[async_trait]
    impl EventProducer for MockBroker {
        async fn publish(&self, _event: &EventEnvelope) -> Result<()> {
            unimplemented!()
        }

        async fn publish_batch(&self, events: &[EventEnvelope]) -> Result<()> {
            if *self.state.should_fail.lock().unwrap() {
                return Err(Error::internal("Simulated broker failure"));
            }
            let mut published = self.state.published.lock().unwrap();
            published.extend_from_slice(events);
            Ok(())
        }
    }

    fn create_dummy_envelope() -> EventEnvelope {
        EventEnvelope {
            id: Uuid::new_v4(),
            region_code: "EU".to_string(),
            aggregate_type: "Test".to_string(),
            aggregate_id: Uuid::new_v4().to_string(),
            event_type: "test.event".to_string(),
            payload: serde_json::json!({}),
            metadata: None,
            occurred_at: chrono::Utc::now(),
        }
    }

    // --- UNIT TESTS ---

    #[tokio::test]
    async fn test_shutdown_immediately_if_signal_already_true() {
        // Arrange
        let store_state = MockStoreState {
            unprocessed: Arc::new(Mutex::new(vec![create_dummy_envelope()])),
            processed_ids: Arc::new(Mutex::new(vec![])),
            fetch_calls: Arc::new(AtomicUsize::new(0)),
            should_fail: Arc::new(Mutex::new(false)),
        };
        let broker_state = MockBrokerState {
            published: Arc::new(Mutex::new(vec![])),
            should_fail: Arc::new(Mutex::new(false)),
        };

        let processor = OutboxProcessor::new(
            MockStore {
                state: store_state.clone(),
            },
            MockBroker {
                state: broker_state.clone(),
            },
            10,
            Duration::from_millis(1),
        );

        // On initialise le canal directement à `true` (demande d'arrêt immédiate)
        let (_tx, rx) = watch::channel(true);

        // Act
        processor.run(rx).await;

        // Assert
        assert_eq!(
            store_state.fetch_calls.load(Ordering::SeqCst),
            0,
            "La boucle aurait dû break avant le premier traitement"
        );
        assert_eq!(store_state.processed_ids.lock().unwrap().len(), 0);
        assert_eq!(broker_state.published.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_nominal_empty_store_polls_and_waits() {
        // Arrange
        let store_state = MockStoreState {
            unprocessed: Arc::new(Mutex::new(vec![])), // Aucun message en attente
            processed_ids: Arc::new(Mutex::new(vec![])),
            fetch_calls: Arc::new(AtomicUsize::new(0)),
            should_fail: Arc::new(Mutex::new(false)),
        };
        let broker_state = MockBrokerState {
            published: Arc::new(Mutex::new(vec![])),
            should_fail: Arc::new(Mutex::new(false)),
        };

        let processor = OutboxProcessor::new(
            MockStore {
                state: store_state.clone(),
            },
            MockBroker {
                state: broker_state,
            },
            10,
            Duration::from_millis(10), // Intervalle court pour le test
        );

        let (tx, rx) = watch::channel(false);

        // Act
        // On lance le processeur en tâche de fond, on le laisse tourner un court instant, puis on l'éteint
        let handle = tokio::spawn(async move {
            processor.run(rx).await;
        });

        tokio::time::sleep(Duration::from_millis(25)).await;
        tx.send(true).unwrap(); // Déclenchement du shutdown
        handle.await.unwrap();

        // Assert
        let calls = store_state.fetch_calls.load(Ordering::SeqCst);
        assert!(
            calls >= 1,
            "Le store aurait dû être interrogé au moins une fois"
        );
        assert_eq!(
            store_state.processed_ids.lock().unwrap().len(),
            0,
            "Rien n'aurait dû être traité"
        );
    }

    #[tokio::test]
    async fn test_fast_looping_when_batch_is_full() {
        // Arrange
        // On configure un batch_size de 2, et on injecte 3 événements (il faudra 2 passages)
        let batch_size = 2;
        let store_state = MockStoreState {
            unprocessed: Arc::new(Mutex::new(vec![
                create_dummy_envelope(),
                create_dummy_envelope(),
                create_dummy_envelope(),
            ])),
            processed_ids: Arc::new(Mutex::new(vec![])),
            fetch_calls: Arc::new(AtomicUsize::new(0)),
            should_fail: Arc::new(Mutex::new(false)),
        };
        let broker_state = MockBrokerState {
            published: Arc::new(Mutex::new(vec![])),
            should_fail: Arc::new(Mutex::new(false)),
        };

        // On configure un long intervalle de polling intentionnellement.
        // Si la logique de boucle rapide fonctionne, le processeur enchaînera le 2ème batch
        // sans attendre ces 5 secondes.
        let processor = OutboxProcessor::new(
            MockStore {
                state: store_state.clone(),
            },
            MockBroker {
                state: broker_state.clone(),
            },
            batch_size,
            Duration::from_secs(5),
        );

        let (tx, rx) = watch::channel(false);

        // Act
        let handle = tokio::spawn(async move {
            processor.run(rx).await;
        });

        // On attend juste un instant le temps que le processeur consomme le backlog fluide
        tokio::time::sleep(Duration::from_millis(20)).await;
        tx.send(true).unwrap(); // Fermeture
        handle.await.unwrap();

        // Assert
        // Le premier batch a traité 2 éléments (égal à batch_size), il a donc bypassé le sleep !
        // Le second batch a traité 1 élément (inférieur à batch_size), il s'est mis en sleep où on l'a intercepté.
        assert_eq!(
            store_state.fetch_calls.load(Ordering::SeqCst),
            2,
            "Le store aurait dû exécuter exactement 2 batchs consécutifs"
        );
        assert_eq!(
            store_state.processed_ids.lock().unwrap().len(),
            3,
            "Les 3 messages doivent être confirmés en DB"
        );
        assert_eq!(
            broker_state.published.lock().unwrap().len(),
            3,
            "Les 3 messages doivent être émis sur le broker"
        );
    }

    #[tokio::test]
    async fn test_resilience_on_broker_failure() {
        // Arrange
        let store_state = MockStoreState {
            unprocessed: Arc::new(Mutex::new(vec![create_dummy_envelope()])),
            processed_ids: Arc::new(Mutex::new(vec![])),
            fetch_calls: Arc::new(AtomicUsize::new(0)),
            should_fail: Arc::new(Mutex::new(false)),
        };

        // Broker configuré pour échouer systématiquement
        let broker_state = MockBrokerState {
            published: Arc::new(Mutex::new(vec![])),
            should_fail: Arc::new(Mutex::new(true)),
        };

        let processor = OutboxProcessor::new(
            MockStore {
                state: store_state.clone(),
            },
            MockBroker {
                state: broker_state.clone(),
            },
            10,
            Duration::from_millis(5),
        );

        let (tx, rx) = watch::channel(false);

        // Act
        let handle = tokio::spawn(async move {
            processor.run(rx).await;
        });

        tokio::time::sleep(Duration::from_millis(15)).await;
        tx.send(true).unwrap();
        handle.await.unwrap();

        // Assert
        assert!(store_state.fetch_calls.load(Ordering::SeqCst) >= 1);
        assert_eq!(
            store_state.processed_ids.lock().unwrap().len(),
            0,
            "La base de données ne doit JAMAIS acquitter un message si le broker a rejeté le batch"
        );
        assert_eq!(broker_state.published.lock().unwrap().len(), 0);
    }
}
