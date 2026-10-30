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
    tls = {
      source  = "hashicorp/tls"
      version = "~> 4"
    }
  }
}

# Set DIGITALOCEAN_TOKEN
provider "digitalocean" {}

# Set CLOUDFLARE_API_TOKEN
provider "cloudflare" {}

data "cloudflare_ip_ranges" "cloudflare_ips" {}

data "cloudflare_zone" "ryansheppard" {
  filter = {
    name = "ryansheppard.xyz"
  }
}

resource "cloudflare_dns_record" "pj" {
  zone_id = data.cloudflare_zone.ryansheppard.zone_id
  name    = "pj.ryansheppard.xyz"
  type    = "A"
  content = digitalocean_droplet.pollingjuegos.ipv4_address
  # Must stay proxied: the firewall only admits Cloudflare's ranges.
  proxied = true
  ttl     = 1
}

resource "tls_private_key" "origin" {
  algorithm   = "ECDSA"
  ecdsa_curve = "P256"
}

resource "tls_cert_request" "origin" {
  private_key_pem = tls_private_key.origin.private_key_pem
  dns_names       = ["pj.ryansheppard.xyz"]

  subject {
    common_name = "pj.ryansheppard.xyz"
  }
}

resource "cloudflare_origin_ca_certificate" "origin" {
  csr                = tls_cert_request.origin.cert_request_pem
  hostnames          = ["pj.ryansheppard.xyz"]
  request_type       = "origin-ecc"
  requested_validity = 5475
}

resource "cloudflare_zone_setting" "ssl" {
  zone_id    = data.cloudflare_zone.ryansheppard.zone_id
  setting_id = "ssl"
  value      = "strict"
}

# A zone-specific AOP client cert instead of Cloudflare's shared one, which any
# Cloudflare customer's proxied hostname would also present.
resource "tls_private_key" "aop" {
  algorithm = "RSA"
  rsa_bits  = 2048
}

resource "tls_self_signed_cert" "aop" {
  private_key_pem       = tls_private_key.aop.private_key_pem
  validity_period_hours = 87600
  allowed_uses          = ["digital_signature", "key_encipherment", "client_auth"]

  subject {
    common_name = "pollingjuegos cloudflare origin pull"
  }
}

resource "cloudflare_authenticated_origin_pulls_certificate" "aop" {
  zone_id     = data.cloudflare_zone.ryansheppard.zone_id
  certificate = tls_self_signed_cert.aop.cert_pem
  private_key = tls_private_key.aop.private_key_pem
}

resource "cloudflare_authenticated_origin_pulls_settings" "aop" {
  zone_id = data.cloudflare_zone.ryansheppard.zone_id
  enabled = true

  depends_on = [cloudflare_authenticated_origin_pulls_certificate.aop]
}

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

output "origin_certificate" {
  value = cloudflare_origin_ca_certificate.origin.certificate
}

output "origin_private_key" {
  value     = tls_private_key.origin.private_key_pem
  sensitive = true
}

output "aop_client_certificate" {
  value = tls_self_signed_cert.aop.cert_pem
}
