% Parameter-efficient fine-tuning in tinygrad

Parameter-efficient fine-tuning (PEFT) is a family of techniques used to adapt
LLMs (along with other types of large models) to specific tasks or datasets.
For example, if you had a private dataset from your legal practice, you could
take [gpt-oss][0] off the shelf and customize it to be really good at your
specific legal specialty. Unlike full fine-tuning (FFT), with PEFT you only
need to update a subset of the weights in the model, leading to better
computational efficiency while requiring significantly less storage.

This holiday season, I decided to implement [Low Rank Adapatation][1] (LoRA)
using [tinygrad][2]. LoRA because I currently work on [Tinker][3] and a deeper
understanding of ML theory and implementation seemed useful. Tinygrad because
it was different enough from pytorch where I would learn more but similar
enough so that any learning would transfer.

## Borrowing from tinygrad

For this exercise I chose llama 3.2 1B to build on, as I had a tinygrad
inference demo to base my work off of, as well as a 1B model being quicker to
iterate on.  Much credit goes to tinygrad for providing this example, as it
would've been a much larger exercise to re-implement the llama architecture by
hand.

#### A nice bug

While getting the tinygrad demo to work as-is, I found a nice bug where the
demo failed to account for llama3 1B and 3B having [tied embeddings][6], for
which I [fixed upstream][7]. It was a small edge case that only triggered on
the smaller models, and only if using Huggingface safetensor checkpoints. I
suspect GGUF, when quantizing, denormalizes the input and output embedding
tensors.

## Towards full fine-tuning

After that, to create a solid starting point, I inlined all the [code from
tinygrad examples][4] into [a single file][5] outside of the tinygrad tree.
Might just be me, but a single file ends up being a log simpler to hack on
that having a bunch of files everywhere.

With that, I started moving the code towards being trainable. Training, at this
point, implies FFT, as weight updates would apply to all layers of the original
base model. It ended up being fairly straightfoward and not missing that many
things, namely:

* Ensuring batch worked, by [forcing batch prefill][10]
* Making [forward pass return logits][11], so I could calculate loss
* Implementing [weighted cross entropy][12] loss
* Removing the bf16 -> [float16 casts][13] to stabilize training
* Adding [greedy][15] [sampling][14]

With the above as well as implementing greedy sampling, I could successfully adapt
llama to speak pig latin, following the general recipe for pig-latin [documented
in the Tinker quickstart][8]:

```
$ uv run main.py --model ~/weights/llama3.2-1b-instruct/ --size 1B
[..]
loaded weights in 2308.98 ms, 2.47 GB loaded at 1.07 GB/s
step 0: loss 9.0799
step 1: loss 5.8170
step 2: loss 3.4434
step 3: loss 2.0425
step 4: loss 1.1673
step 5: loss 0.5927
step 6: loss 0.2497

--- Inference ---
Input: hello world
Output: ellohay orldway
```

Note the learning rate is 10x higher than with Tinker, as Tinker uses LoRA
and learning rates for LoRA need to be 10x higher than FFT, as documented
in [LoRA without Regrest][9].

## Implementing LoRA


[0]: https://openai.com/index/introducing-gpt-oss/
[1]: https://arxiv.org/abs/2106.09685
[2]: https://github.com/tinygrad/tinygrad
[3]: https://thinkingmachines.ai/tinker/
[4]: https://github.com/tinygrad/tinygrad/blob/2bb07d48241bc750b54b91938db409358c290ea6/examples/llama3.py
[5]: https://github.com/danobi/tinygrad-llama3/blob/e3b22823b3edf18fbc0dce3dff98d95f72798d19/main.py
[6]: https://arxiv.org/abs/1608.05859
[7]: https://github.com/tinygrad/tinygrad/commit/4edaaf19e5598c6e5325ffae263ae470ec2db6d3
[8]: https://tinker-docs.thinkingmachines.ai/training-sampling
[9]: https://thinkingmachines.ai/blog/lora/#optimal-learning-rates-for-lora-vs-fullft
[10]: https://github.com/danobi/tinygrad-llama3/commit/1cb29d418dbdbc8c96357c045f53d5ffee926b03
[11]: https://github.com/danobi/tinygrad-llama3/commit/702c2f13a71c06166b1060e5ef2359801d11772b
[12]: https://github.com/danobi/tinygrad-llama3/commit/30cc484311dc31261ba6313148f1e8008f535951
[13]: https://github.com/danobi/tinygrad-llama3/commit/3543b5eea05b3d672f79e39dae901c0fda96e80d
[14]: https://github.com/danobi/tinygrad-llama3/commit/d6948756f92018d9d66388dfd23368e95b1397ed
[15]: https://github.com/danobi/tinygrad-llama3/commit/3f49d4cbd3c6035248d5bee23eaf5622e2e4f62d
