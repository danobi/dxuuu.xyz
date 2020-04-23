#!/bin/env bash

# This script updates the letsencrypt https cert.

set -e

certbot renew
cat \
  /etc/letsencrypt/live/dxuuu.xyz/{privkey,cert}.pem > \
  /var/www/dxuuu.xyz/.letsencrypt/ssl.pem
systemctl restart lighttpd.service
