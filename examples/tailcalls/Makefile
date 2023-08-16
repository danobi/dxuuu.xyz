SRCS := $(wildcard *.dot)
OUTS := $(SRCS:.dot=.svg)

all: $(OUTS)

%.svg: %.dot
	dot -T svg $< -o $@
