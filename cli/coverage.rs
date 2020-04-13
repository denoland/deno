use url::Url;

pub struct CoverageCollector {
    url: Url,
}

impl CoverageCollector {
    pub fn new(url: Url) -> Self {
        Self {
            url,
        }
    }

    pub fn connect(&self) {
        todo!()
    }

    pub fn start_collecting(&self) {
        todo!()
    }

    pub fn stop_collecting(&self) {
        todo!()
    }

    pub fn get_report(&self) -> String {
        todo!()
    }
}


// pub fn run_coverage_collector_thread(

// ) -> Result<(JoinHandle<()>, WebWorkerHandle), ErrBox> {
//     let (handle_sender, handle_receiver) =
//     std::sync::mpsc::sync_channel::<Result<WebWorkerHandle, ErrBox>>(1);

//     let builder =
//     std::thread::Builder::new().name("deno-coverage-collector".to_string());

//     let join_handle = std::thread::spawn(|| {
//         let fut = async move {
//           let (socket, response) = tokio_tungstenite::connect_async(inspector_url)
//             .await
//             .expect("Can't connect");
//           assert_eq!(response.status(), 101);
    
//           let mut msg_id = 1;
    
//           let (mut socket_tx, mut socket_rx) = socket.split();
    
//           // let test_steps = vec![
//           //   WsSend(r#"{"id":1,"method":"Runtime.enable"}"#),
//           //   WsSend(r#"{"id":2,"method":"Profiler.enable"}"#),
//           //   WsSend(
//           //     r#"{"id":3,"method":"Profiler.startPreciseCoverage", "params": {"callCount": false, "detailed": true } }"#,
//           //   ),
//           //   WsRecv(
//           //     r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#,
//           //   ),
//           //   WsRecv(r#"{"id":1,"result":{}}"#),
//           //   WsRecv(r#"{"id":2,"result":{}}"#),
//           //   WsRecv(r#"{"id":3,"result":{"timestamp":"#),
//           //   WsSend(r#"{"id":4,"method":"Runtime.runIfWaitingForDebugger"}"#),
//           //   WsRecv(r#"{"id":4,"result":{}}"#),
//           //   StdOut("hello a"),
//           //   StdOut("hello b"),
//           //   StdOut("hello b"),
//           //   WsSend(r#"{"id":5,"method":"Profiler.takePreciseCoverage"}"#),
//           //   WsSend(r#"{"id":6,"method":"Profiler.stopPreciseCoverage"}"#),
//           //   WsRecv(r#"{"id":5,"result":{"result":[{"#),
//           // ];
    
//           socket_tx.send(r#"{"id":1,"method":"Runtime.enable"}"#).await.unwrap();
//           socket_tx.send(r#"{"id":2,"method":"Profiler.enable"}"#).await.unwrap();
//           socket_tx.send(r#"{"id":3,"method":"Profiler.startPreciseCoverage", "params": {"callCount": false, "detailed": true } }"#).await.unwrap();
    
//         }.boxed_local();
    
//         tokio_util::run_basic(fut)
//       });

// }


