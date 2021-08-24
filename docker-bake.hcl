variable "IMAGE_NAME" {
  default = "denoland/deno"
}

group "default" {
  targets = ["debian"]
}

group "all" {
  targets = ["debian", "ubuntu", "alpine", "centos"]
}

target "base" {
  context = "."
}

target "debian" {
  inherits = ["base"]
  tags = ["${IMAGE_NAME}:latest", "${IMAGE_NAME}:debian"]
}

target "ubuntu" {
  inherits = ["base"]
  tags = ["${IMAGE_NAME}:ubuntu"]
}

target "alpine" {
  inherits = ["base"]
  tags = ["${IMAGE_NAME}:alpine"]
}

target "centos" {
  inherits = ["base"]
  tags = ["${IMAGE_NAME}:centos"]
}

target "bin" {
  inherits = ["base"]
  tags = ["denoland/deno:bin"]
  target = "bin"
}

target "export-bin" {
  inherits = ["bin"]
  output = ["."]
}
