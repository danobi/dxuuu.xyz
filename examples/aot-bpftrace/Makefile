all: old-architecture.png

%.png: %.mscgen
	mscgen -T png -i $< -o $@

clean:
	rm -f *.png

.PHONY: all clean
