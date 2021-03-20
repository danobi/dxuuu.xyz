# dxuuu.xyz

This is the source and infrastructure for my website.

## Notes

* `src/` holds all the markdown source files for the website.

* `html/` is the build directory with the compiled source files

* The Makefile will build markdown files from within `src/` (and ending in .md)
and put them in `html/`

* `scripts/` holds an update script used by the systemd service files

* This site is kind of flakey in terms of configuration. I'll try to improve the
infrastructure when I get more time.


## Installation

* Clone this repository onto the server in `$HOME`
    * It **has** to be at `$HOME` since the systemd service scripts are
    currently hard coded to that location

* Make a user owned (not http server user owned) directory in `/var/www/dxuuu.xyz`

* Set up the systemd timer for automated builds and site updates:
```
$ cd dxuuu.xyz/scripts
$ systemctl --user link `realpath ./update-site.service`
$ systemctl --user link `realpath ./update-site.timer`
$ systemctl --user enable update-site.timer update-site.service
$ systemctl --user start update-site.timer update-cert.timer

$ # Start caddy webserver
$ cd ..
$ sudo cp Caddyfile /etc/caddy/Caddyfile
$ sudo mkdir /var/log/caddy
$ sudo chown caddy:caddy /var/log/caddy
$ sudo systemctl enable caddy
$ sudo systemctl start caddy
```
