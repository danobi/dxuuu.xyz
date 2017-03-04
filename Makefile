# Converts src/*.md to HTML documents in output/*.html

# glob all the source files
SOURCE_DOCS := $(wildcard src/*.md)
# first remove directory prefixes and then add 'output' directory prefix
EXPORTED_DOCS = $(addprefix html/,$(notdir $(SOURCE_DOCS:.md=.html)))
PANDOC = /bin/env pandoc
PANDOC_OPTIONS = --smart --standalone
PANDOC_HTML_OPTIONS = --to html5

html/%.html : src/%.md
	$(PANDOC) $(PANDOC_OPTIONS) $(PANDOC_HTML_OPTIONS) $< -o $@

.PHONY: all clean

all : $(EXPORTED_DOCS)

clean:
	rm $(EXPORTED_DOCS)
