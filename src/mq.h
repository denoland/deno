
#ifndef MQ_H_
#define MQ_H_

#include <assert.h>
#include <mutex>
#include "deno.h"

namespace deno {

class MessageQueue {
  struct Message {
    deno_buf buf = DENO_BUF_INIT;
    Message* next;
  };

  typedef std::mutex Mutex;

  Message* head_;
  Message* tail_;

  Mutex mutex_;
  std::condition_variable cv_;
  bool reader_is_blocked_;

 public:
  MessageQueue()
      : head_(nullptr),
        tail_(nullptr),
        mutex_(),
        cv_(),
        reader_is_blocked_(false) {}

  void Send(deno_buf* buf, bool nowake = false) {
    auto m = new Message;
    deno_buf_move_into(&m->buf, buf);

    std::unique_lock<Mutex> lock(mutex_);

    if (head_ == nullptr) {
      m->next = nullptr;
      head_ = m;
      tail_ = m;
    } else {
      m->next = head_;
      head_ = m;
    }

    if (!nowake && reader_is_blocked_) {
      reader_is_blocked_ = false;
      lock.unlock();  // Optimization.
      cv_.notify_one();
    }
  }

  // TODO: should take a timeout value.
  bool Recv(deno_buf* buf_out, bool nowait = false) {
    std::unique_lock<Mutex> lock(mutex_);

    if (nowait && head_ == nullptr) {
      return false;
    }

    while (head_ == nullptr) {
      reader_is_blocked_ = true;
      cv_.wait(lock);
    }

    auto m = head_;

    head_ = m->next;
    if (m == tail_) {
      tail_ = nullptr;
    }

    deno_buf_move_into(buf_out, &m->buf);
    delete m;
    return true;
  }

  typedef std::function<bool(const deno_buf&)> FilterCallback;

  bool RecvFilter(deno_buf* buf_out, const FilterCallback& filter) {
    std::unique_lock<Mutex> lock(mutex_);

    while (head_ == nullptr) {
      reader_is_blocked_ = true;
      cv_.wait(lock);
    }

    Message* m = head_;
    Message* prev = nullptr;

    while (!filter(m->buf)) {
      prev = m;
      while (m->next == nullptr) {
        reader_is_blocked_ = true;
        cv_.wait(lock);
      }
      m = m->next;
    }

    if (prev != nullptr) {
      prev->next = m->next;
    } else {
      assert(m == head_);
      head_ = m->next;
    }
    if (m == tail_) {
      tail_ = prev;
    }

    deno_buf_move_into(buf_out, &m->buf);
    delete m;
    return true;
  }
};

}  // namespace deno

#endif  // MQ_H_
