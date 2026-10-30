terraform {
  required_version = ">= 1.5.0"

  required_providers {
    digitalocean = {
      source  = "digitalocean/digitalocean"
      version = "~> 2.0"
    }
    cloudflare = {
      source  = "cloudflare/cloudflare"
      version = "~> 5"
    }
  }
}

# Set DIGITALOCEAN_TOKEN
provider "digitalocean" {}

# Set CLOUDFLARE_API_KEY
provider "cloudflare" {}

data "cloudflare_ip_ranges" "cloudflare_ips" {}

data "digitalocean_ssh_key" "arch" {
  name = "arch"
}

resource "digitalocean_droplet" "pollingjuegos" {
  name       = "pollingjuegos"
  region     = "nyc3"
  size       = "s-1vcpu-1gb"
  image      = "ubuntu-24-04-x64"
  ssh_keys   = [data.digitalocean_ssh_key.arch.id]
  monitoring = true
  backups    = var.enable_droplet_backups
  tags       = ["pollingjuegos"]
}

# DigitalOcean registry names are account-wide; the Starter tier is free but storage-limited.
resource "digitalocean_container_registry" "pollingjuegos" {
  name                   = "pollingjuegos"
  subscription_tier_slug = "starter"
}

resource "digitalocean_firewall" "pollingjuegos" {
  name        = "pollingjuegos"
  droplet_ids = [digitalocean_droplet.pollingjuegos.id]

  inbound_rule {
    protocol   = "tcp"
    port_range = "443"
    source_addresses = concat(
      data.cloudflare_ip_ranges.cloudflare_ips.ipv4_cidrs,
      data.cloudflare_ip_ranges.cloudflare_ips.ipv6_cidrs,
    )
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

output "registry_endpoint" {
  value = digitalocean_container_registry.pollingjuegos.endpoint
}
