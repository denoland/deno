fn op_is_proxy(value: serde_v8::Value) -> bool {
  value.v8_value.is_proxy()
}
