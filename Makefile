SOURCE_DOCS := $(wildcard src/*.md)
SOURCE_CSS := css/pandoc.css

EXPORTED_DOCS := $(addprefix html/,$(notdir $(SOURCE_DOCS:.md=.html)))
EXPORTED_CSS := $(addprefix html/css/,$(notdir $(SOURCE_CSS)))
PANDOC := podman run --rm -v $(shell pwd):/data --userns=keep-id pandoc/core
PANDOC_OPTIONS := -t markdown-smart --standalone
PANDOC_HTML_OPTIONS := --to html5 --css $(SOURCE_CSS)

html/%.html : src/%.md $(EXPORTED_CSS) Makefile
	$(PANDOC) $(PANDOC_OPTIONS) $(PANDOC_HTML_OPTIONS) $< -o $@

html/css/%.css: css/%.css Makefile
	cp $< $@

.PHONY: all install clean

all: $(EXPORTED_DOCS) $(EXPORTED_CSS) html/atom.xml

html/atom.xml: $(EXPORTED_DOCS)
	cd atom; cargo build
	./atom/target/debug/atom --html-dir ./html --repo-root . > html/atom.xml

install:
	rm -rf /var/www/dxuuu.xyz/*
	[[ -d /var/www/dxuuu.xyz/css ]] || mkdir /var/www/dxuuu.xyz/css
	cp -r html/* /var/www/dxuuu.xyz/
	cp -r examples /var/www/dxuuu.xyz
	cp assets/favicon.ico /var/www/dxuuu.xyz

clean:
	rm -f html/*.html
	rm -f html/css/*.css
	rm -f html/*.xml
	cd atom; cargo clean
