SOURCE_DOCS := $(wildcard src/*.md)
SOURCE_CSS := css/pandoc.css
SOURCE_ATOM := $(wildcard atom/src/*.rs)

EXPORTED_DOCS := $(addprefix html/,$(notdir $(SOURCE_DOCS:.md=.html)))
EXPORTED_CSS := $(addprefix html/css/,$(notdir $(SOURCE_CSS)))
PANDOC_VERSION := 3.1.1
PANDOC := podman run --rm -v $(shell pwd):/data --userns=keep-id pandoc/core:$(PANDOC_VERSION)
PANDOC_OPTIONS := -t markdown-smart --standalone
PANDOC_HTML_OPTIONS := --to html5 --css $(SOURCE_CSS)

.PHONY: all
all: $(EXPORTED_DOCS) html/atom.xml

html:
	mkdir -p html/css

html/%.html : src/%.md $(EXPORTED_CSS) | html
	$(PANDOC) $(PANDOC_OPTIONS) $(PANDOC_HTML_OPTIONS) $< -o $@

html/css/%.css: css/%.css | html
	cp $< $@

html/atom.xml: $(EXPORTED_DOCS) $(SOURCE_ATOM) | html
	cd atom; cargo build
	./atom/target/debug/atom --html-dir ./html --repo-root . > $@ || rm $@

.PHONY: install
install: all
	rm -rf /var/www/dxuuu.xyz/*
	[[ -d /var/www/dxuuu.xyz/css ]] || mkdir /var/www/dxuuu.xyz/css
	cp -r html/* /var/www/dxuuu.xyz/
	cp -r examples /var/www/dxuuu.xyz
	cp assets/favicon.ico /var/www/dxuuu.xyz

.PHONY: clean
clean:
	rm -rf html
	cd atom; cargo clean
