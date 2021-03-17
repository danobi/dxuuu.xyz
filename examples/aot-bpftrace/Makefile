all: old-architecture.png \
	new-architecture.png \
	old-passes.png \
	new-passes.png \
	aot-execution.png

%.png: %.mscgen
	mscgen -T png -i $< -o $@

clean:
	rm -f *.png

.PHONY: all clean
