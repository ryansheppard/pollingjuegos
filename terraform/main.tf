terraform {
  required_version = ">= 1.5.0"

  required_providers {
    digitalocean = {
      source  = "digitalocean/digitalocean"
      version = "~> 2.0"
    }
  }
}

# Set DIGITALOCEAN_TOKEN in the environment; never pass secrets in -var files.
provider "digitalocean" {}

data "digitalocean_ssh_key" "arch" {
  name = "arch"
}

resource "digitalocean_droplet" "pollingjuegos" {
  name       = "pollingjuegos"
  region     = "nyc3"
  size       = "s-1vcpu-512mb-10gb"
  image      = "ubuntu-24-04-x64"
  ssh_keys   = [data.digitalocean_ssh_key.arch.id]
  monitoring = true
  backups    = var.enable_droplet_backups
  tags       = ["pollingjuegos"]
}

resource "digitalocean_firewall" "pollingjuegos" {
  name        = "pollingjuegos"
  droplet_ids = [digitalocean_droplet.pollingjuegos.id]

  inbound_rule {
    protocol         = "tcp"
    port_range       = "80"
    source_addresses = ["0.0.0.0/0", "::/0"]
  }

  inbound_rule {
    protocol         = "tcp"
    port_range       = "443"
    source_addresses = ["0.0.0.0/0", "::/0"]
  }

  outbound_rule {
    protocol              = "tcp"
    port_range            = "1-65535"
    destination_addresses = ["0.0.0.0/0", "::/0"]
  }

  outbound_rule {
    protocol              = "udp"
    port_range            = "1-65535"
    destination_addresses = ["0.0.0.0/0", "::/0"]
  }

  outbound_rule {
    protocol              = "icmp"
    destination_addresses = ["0.0.0.0/0", "::/0"]
  }
}

variable "enable_droplet_backups" {
  description = "Enable DigitalOcean's paid droplet snapshots (not a substitute for offsite SQLite backups)."
  type        = bool
  default     = false
}

output "ipv4_address" {
  value = digitalocean_droplet.pollingjuegos.ipv4_address
}
