SOURCE_DOCS := $(wildcard src/*.md)
SOURCE_CSS := css/pandoc.css
SOURCE_ATOM := $(wildcard atom/src/*.rs)

EXPORTED_DOCS := $(addprefix html/,$(notdir $(SOURCE_DOCS:.md=.html)))
EXPORTED_CSS := $(addprefix html/css/,$(notdir $(SOURCE_CSS)))
PANDOC_VERSION := 3.1.1
PANDOC := podman run --rm -v $(shell pwd):/data --userns=keep-id pandoc/core:$(PANDOC_VERSION)
PANDOC_OPTIONS := -t markdown-smart --standalone
PANDOC_HTML_OPTIONS := --to html5 --css $(SOURCE_CSS)
PANDOC_POST_HTML_OPTIONS := --include-in-header assets/header.html

INSTALL_DIR ?= /var/www/dxuuu.xyz

.PHONY: all
all: $(EXPORTED_DOCS) html/atom.xml

html:
	mkdir -p html/css

html/index.html: src/index.md $(EXPORTED_CSS) | html
	$(PANDOC) $(PANDOC_OPTIONS) $(PANDOC_HTML_OPTIONS) $< -o $@

html/%.html : src/%.md $(EXPORTED_CSS) | html
	$(PANDOC) $(PANDOC_OPTIONS) $(PANDOC_HTML_OPTIONS) $(PANDOC_POST_HTML_OPTIONS) $< -o $@

html/css/%.css: css/%.css | html
	cp $< $@

html/atom.xml: $(EXPORTED_DOCS) $(SOURCE_ATOM) | html
	cd atom; cargo build
	./atom/target/debug/atom --html-dir ./html --repo-root . > $@ || rm $@

.PHONY: install
install: all
	rm -rf $(INSTALL_DIR)/*
	[[ -d $(INSTALL_DIR)/css ]] || mkdir $(INSTALL_DIR)/css
	cp -r html/* $(INSTALL_DIR)/
	cp -r examples $(INSTALL_DIR)
	cp -r resources $(INSTALL_DIR)/r
	cp assets/favicon.ico $(INSTALL_DIR)

.PHONY: clean
clean:
	rm -rf html
	cd atom; cargo clean
