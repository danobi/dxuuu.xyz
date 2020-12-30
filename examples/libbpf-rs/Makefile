all: bcc.svg core.svg

build/bin/svgbob:
	$(shell which svgbob &> /dev/null || cargo install svgbob_cli --help)

%.svg: %.ascii build/bin/svgbob
	svgbob $< > $@

clean:
	rm -f *.svg

.PHONY: all clean
