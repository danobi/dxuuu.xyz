SOURCE_DOCS := $(wildcard src/*.md)
SOURCE_CSS := css/pandoc.css

EXPORTED_DOCS := $(addprefix html/,$(notdir $(SOURCE_DOCS:.md=.html)))
EXPORTED_CSS := $(addprefix html/css/,$(notdir $(SOURCE_CSS)))
PANDOC := /bin/env pandoc
PANDOC_OPTIONS := -t markdown-smart --standalone
PANDOC_HTML_OPTIONS := --to html5 --css $(SOURCE_CSS)

html/%.html : src/%.md $(EXPORTED_CSS) Makefile
	$(PANDOC) $(PANDOC_OPTIONS) $(PANDOC_HTML_OPTIONS) $< -o $@

html/css/%.css: css/%.css Makefile
	cp $< $@

.PHONY: all install clean

all: $(EXPORTED_DOCS) $(EXPORTED_CSS)

install:
	rm -r /var/www/dxuuu.xyz/*
	[[ -d /var/www/dxuuu.xyz/css ]] || mkdir /var/www/dxuuu.xyz/css
	cp -r html/* /var/www/dxuuu.xyz/
	cp -r examples /var/www/dxuuu.xyz
	cp assets/favicon.ico /var/www/dxuuu.xyz

clean:
	rm -f html/*.html
	rm -f html/css/*.css
