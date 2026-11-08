variable "runner_image_tag" {
  type    = string
  default = "latest"
}

resource "digitalocean_microvm" "runner" {
  name   = "pollingjuegos-runner"
  region = "mkc1"

  size {
    cpu    = 8
    memory = 16384
  }

  source {
    oci_ref = "${digitalocean_container_registry.pollingjuegos.endpoint}/runner:${var.runner_image_tag}"
  }

  auto_pause {
    enabled      = true
    idle_timeout = "15m0s"
  }

  auto_resume     = false
  ingress_enabled = false

  tags = ["pollingjuegos"]

  lifecycle {
    ignore_changes = [state]
  }
}

output "runner_microvm_id" {
  value = digitalocean_microvm.runner.id
}
