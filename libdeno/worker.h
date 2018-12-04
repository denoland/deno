#include "third_party/v8/include/v8.h"

namespace deno {


class SerializationDataQueue {
 public:
//  void Enqueue(std::unique_ptr<SerializationData> data);
//  bool Dequeue(std::unique_ptr<SerializationData>* data);
  bool IsEmpty();
  void Clear();

 private:
  v8::base::Mutex mutex_;
//  std::vector<std::unique_ptr<SerializationData>> data_;
};

class Worker {
 public:
  Worker();
  ~Worker();

  // Run the given script on this Worker. This function should only be called
  // once, and should only be called by the thread that created the Worker.
  void StartExecuteInThread(const char* script);
//  // Post a message to the worker's incoming message queue. The worker will
//  // take ownership of the SerializationData.
//  // This function should only be called by the thread that created the Worker.
//  void PostMessage(std::unique_ptr<SerializationData> data);
//  // Synchronously retrieve messages from the worker's outgoing message queue.
//  // If there is no message in the queue, block until a message is available.
//  // If there are no messages in the queue and the worker is no longer running,
//  // return nullptr.
//  // This function should only be called by the thread that created the Worker.
//  std::unique_ptr<SerializationData> GetMessage();
//  // Terminate the worker's event loop. Messages from the worker that have been
//  // queued can still be read via GetMessage().
//  // This function can be called by any thread.
//  void Terminate();
//  // Terminate and join the thread.
//  // This function can be called by any thread.
//  void WaitForThread();

 private:
  class WorkerThread : public v8::base::Thread {
   public:
    explicit WorkerThread(Worker* worker)
        : v8::base::Thread(v8::base::Thread::Options("WorkerThread")),
          worker_(worker) {}

    void Run() override { worker_->ExecuteInThread(); }

   private:
    Worker* worker_;
  };

  void ExecuteInThread();
//  static void PostMessageOut(const v8::FunctionCallbackInfo<v8::Value>& args);

  v8::base::Semaphore in_semaphore_;
  v8::base::Semaphore out_semaphore_;
  SerializationDataQueue in_queue_;
  SerializationDataQueue out_queue_;
  v8::base::Thread* thread_;
  char* script_;
  v8::base::Atomic32 running_;
};

  void WorkerNew(const v8::FunctionCallbackInfo<v8::Value>& args);
//   static void WorkerPostMessage(
//       const v8::FunctionCallbackInfo<v8::Value>& args);
//   static void WorkerGetMessage(const v8::FunctionCallbackInfo<v8::Value>& args);
//   static void WorkerTerminate(const v8::FunctionCallbackInfo<v8::Value>& args);
}