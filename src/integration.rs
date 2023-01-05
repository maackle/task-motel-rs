#![cfg(feature = "test-util")]

use std::sync::Arc;

use crate::*;
use futures::StreamExt;
use tokio::sync::Mutex;

#[tokio::test(flavor = "multi_thread")]
async fn integration() {
    #[derive(Debug, Clone, Hash, PartialEq, Eq)]
    enum GroupKey {
        Root,
        Branch(u8),
        Leaf(u8, u8),
    }

    let mut tm: TaskManager<GroupKey, String> = TaskManager::default();

    fn blocker(mut stop_rx: StopSignal) -> Task<String> {
        Task {
            handle: tokio::spawn(async move {
                stop_rx.await;
                dbg!("stopped");
                Ok(())
            }),
            info: "blocker".to_string(),
        }
    }

    fn triggered(mut stop_rx: StopSignal, trigger_rx: StopSignal) -> Task<String> {
        let handle = tokio::spawn(async move {
            futures::future::select(stop_rx, trigger_rx).await;
            Ok(())
        });
        Task {
            handle,
            info: "triggered".to_string(),
        }
    }

    let trigger = StopBroadcaster::new();

    {
        use GroupKey::*;

        tm.add_group(Root, None);
        tm.add_group(Branch(1), None);
        tm.add_group(Branch(2), None);
        tm.add_group(Leaf(1, 1), None);
        tm.add_group(Leaf(1, 2), None);
        tm.add_group(Leaf(2, 1), None);
        tm.add_group(Leaf(2, 2), None);
        //    tm.add_group(Root, None);
        //         tm.add_group(Branch(1), Some(Root));
        //         tm.add_group(Branch(2), Some(Root));
        //         tm.add_group(Leaf(1, 1), Some(Branch(1)));
        //         tm.add_group(Leaf(1, 2), Some(Branch(1)));
        //         tm.add_group(Leaf(2, 1), Some(Branch(2)));
        //         tm.add_group(Leaf(2, 2), Some(Branch(2)));

        tm.add_task(&Root, |stop| blocker(stop)).await.unwrap();

        tm.add_task(&Branch(1), |stop| triggered(stop, trigger.receiver()))
            .await
            .unwrap();
        tm.add_task(&Branch(2), |stop| blocker(stop)).await.unwrap();

        tm.add_task(&Leaf(1, 1), |stop| triggered(stop, trigger.receiver()))
            .await
            .unwrap();
        tm.add_task(&Leaf(1, 2), |stop| triggered(stop, trigger.receiver()))
            .await
            .unwrap();
        tm.add_task(&Leaf(2, 1), |stop| blocker(stop))
            .await
            .unwrap();
        tm.add_task(&Leaf(2, 2), |stop| blocker(stop))
            .await
            .unwrap();

        // dbg!(&tm);

        let tm = Arc::new(Mutex::new(tm));
        let tm2 = tm.clone();

        let check = tokio::spawn(async move {
            let t = tokio::time::Duration::from_millis(1000);
            dbg!("hi");
            loop {
                match dbg!(tokio::time::timeout(t, tm2.lock().await.next()).await) {
                    Ok(Some(item)) => {
                        dbg!(item);
                    }
                    Ok(None) => break,
                    Err(_) => {
                        println!("*********\n*********\n TIMEOUT\n*********\n*********\n");
                        break;
                    }
                }
            }
            let results: Vec<_> = Arc::try_unwrap(tm2).unwrap().into_inner().collect().await;
            dbg!(results);
        });

        dbg!();
        tm.lock().await.stop_all().unwrap();
        dbg!();
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        dbg!();

        assert_eq!(
            tm.lock().await.groups.keys().cloned().collect::<Vec<_>>(),
            vec![Root]
        );
        dbg!();

        drop(trigger);

        check.await.unwrap();
    }
}
