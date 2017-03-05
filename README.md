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

* Clone this repository onto the server at `/home/daniel`
    * It **has** to be at `/home/daniel` since the systemd service scripts are
    currently hard coded to that location

* Make a user owned (not http server user owned) directory in /var/www/dxuuu.xyz

* Set up the systemd timer for automated builds and site updates
    * `cd dxuuu.xyz/scripts`
    * `sudo ln -s ${PWD}/update-site.timer /etc/systemd/system/update-site.timer`
    * `sudo ln -s ${PWD}/update-site.timer /etc/systemd/system/update-site.service`
    * `sudo systemctl enable update-site.timer`


