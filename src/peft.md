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

All work was done in: https://github.com/danobi/tinygrad-llama3.

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

Ensuring batching worked, by forcing batch prefill:

```diff
diff --git a/main.py b/main.py
index ad24ef9..4733f74 100644
--- a/main.py
+++ b/main.py
@@ -365,10 +365,9 @@ def prefill(model, toks, start_pos=0):
     last_seen_toks = toks
     toks = toks[i:]

-  # prefill the model
-  for tok in tqdm(toks):
-    model(Tensor([[tok]], device=device), start_pos, TEMPERATURE, TOP_K, TOP_P, ALPHA_F, ALPHA_P).realize()
-    start_pos += 1
+  # batch prefill the model
+  model(Tensor([toks], device=device), start_pos, TEMPERATURE, TOP_K, TOP_P, ALPHA_F, ALPHA_P).realize()
+  start_pos += len(toks)
   return start_pos

 if __name__ == "__main__":
```

Making forward pass return logits, so I could calculate loss:

```diff
diff --git a/main.py b/main.py
index 4733f74..a44f49e 100644
--- a/main.py
+++ b/main.py
@@ -195,23 +195,20 @@ def __init__(self, dim:int, hidden_dim:int, n_heads:int, n_layers:int, norm_eps:
     self.freqs_cis = precompute_freqs_cis(dim // n_heads, self.max_context * 2, rope_theta).contiguous().requires_grad_(False)
     self.forward_jit = TinyJit(self.forward) if jit else None

-  def forward(self, tokens:Tensor, start_pos:Union[Variable,int], temperature:float, top_k:int, top_p:float, alpha_f:float, alpha_p:float):
+  def forward(self, tokens:Tensor, start_pos:Union[Variable,int]):
     _bsz, seqlen = tokens.shape
     h = self.tok_embeddings(tokens)
     freqs_cis = self.freqs_cis.cast(h.dtype)[:, start_pos:start_pos+seqlen, :, :, :]

     mask = Tensor.full((1, 1, seqlen, start_pos+seqlen), float("-inf"), dtype=h.dtype, device=h.device).triu(start_pos+1) if seqlen > 1 else None
     for layer in self.layers: h = layer(h, start_pos, freqs_cis, mask)
-    logits = self.output(self.norm(h))
-    if math.isnan(temperature): return logits
+    return self.output(self.norm(h))

-    return sample(logits[:, -1, :].flatten(), temperature, top_k, top_p, alpha_f, alpha_p)
-
-  def __call__(self, tokens:Tensor, start_pos:int, temperature:float=0.0, top_k:int=0, top_p:float=0.8, alpha_f:float=0.0, alpha_p:float=0.0):
+  def __call__(self, tokens:Tensor, start_pos:int):
     # TODO: better way to handle the first call v.s. the rest?
     if tokens.shape[0:2] == (1,1) and self.forward_jit is not None and start_pos != 0:
-      return self.forward_jit(tokens, Variable("start_pos", 1, self.max_context-1).bind(start_pos), temperature, top_k, top_p, alpha_f, alpha_p)
-    return self.forward(tokens, start_pos, temperature, top_k, top_p, alpha_f, alpha_p)
+      return self.forward_jit(tokens, Variable("start_pos", 1, self.max_context-1).bind(start_pos))
+    return self.forward(tokens, start_pos)

 # *** helpers ***

@@ -366,7 +363,7 @@ def prefill(model, toks, start_pos=0):
     toks = toks[i:]

   # batch prefill the model
-  model(Tensor([toks], device=device), start_pos, TEMPERATURE, TOP_K, TOP_P, ALPHA_F, ALPHA_P).realize()
+  model(Tensor([toks], device=device), start_pos).realize()
   start_pos += len(toks)
   return start_pos

@@ -399,8 +396,8 @@ def encode_message(role: str, content: str):
     start_pos = prefill(model, toks[:-1], start_pos=start_pos)
     last_tok = toks[-1]
     while True:
-      tok = model(Tensor([[last_tok]], device=device), start_pos, TEMPERATURE, TOP_K, TOP_P, ALPHA_F, ALPHA_P)
-      tok = tok.item()
+      logits = model(Tensor([[last_tok]], device=device), start_pos)
+      tok = sample(logits[:, -1, :].flatten(), TEMPERATURE, TOP_K, TOP_P, ALPHA_F, ALPHA_P).item()
       start_pos += 1
       last_tok = tok
       if tok in tokenizer.stop_tokens: break
```

From here, implementing weighted cross entropy loss was fairly mechanical,
along with initializing an optimizer and stepping over example data (the latter
shown below):

```diff
diff --git a/main.py b/main.py
index fa58e71..8bdebee 100644
--- a/main.py
+++ b/main.py
@@ -424,24 +424,41 @@ def train(model:Transformer, args:argparse.Namespace):
   padded = [s + [tokenizer.eos_id] * (max_len - len(s)) for s in sequences]
   tokens = Tensor(padded, device=device)  # (B, max_len)

-  # forward pass
-  logits = model(tokens, start_pos=0)  # (B, max_len, vocab_size)
-
-  # shift to align predictions with targets
-  pred = logits[:, :-1, :]   # (B, max_len-1, vocab_size)
-  target = tokens[:, 1:]     # (B, max_len-1)
-
-  # create mask: 1 for output tokens, 0 for prompt and padding
-  # mask[i,j] = 1 if prompt_lens[i]-1 <= j < seq_lens[i]-1
+  # compute weights mask (to not train on prompt)
   seq_lens = Tensor([len(s) for s in sequences], device=device).reshape(-1, 1)  # (B, 1)
   prompt_lens_t = Tensor(prompt_lens, device=device).reshape(-1, 1)             # (B, 1)
   j = Tensor.arange(max_len - 1, device=device).reshape(1, -1)                  # (1, max_len-1)
   mask = (j >= (prompt_lens_t - 1)) * (j < (seq_lens - 1))                      # (B, max_len-1)

-  per_token_loss = pred.flatten(0, 1).cross_entropy(target.flatten(), reduction="none")  # (B*(max_len-1),)
-  per_token_loss = per_token_loss.reshape(len(sequences), max_len - 1)  # (B, max_len-1)
-  loss = (per_token_loss * mask).sum() / mask.sum()
-  print(f"loss: {loss.item():.4f}")
+  # targets don't change between iterations
+  target = tokens[:, 1:]     # (B, max_len-1)
+
+  # create optimizer (only include params that require grad)
+  optim = nn.optim.AdamW(get_parameters(model), lr=1e-5)
+
+  @Tensor.train()
+  def step() -> Tensor:
+    # forward pass
+    logits = model.forward(tokens, start_pos=0)  # (B, max_len, vocab_size)
+
+    # shift to align predictions with targets
+    pred = logits[:, :-1, :]   # (B, max_len-1, vocab_size)
+
+    # compute loss
+    per_token_loss = pred.flatten(0, 1).cross_entropy(target.flatten(), reduction="none")  # (B*(max_len-1),)
+    per_token_loss = per_token_loss.reshape(len(sequences), max_len - 1)  # (B, max_len-1)
+    loss = (per_token_loss * mask).sum() / mask.sum()
+
+    # optimization step
+    optim.zero_grad()
+    loss.backward()
+    optim.step()
+
+    return loss
+
+  for idx in range(6):
+    loss = step()
+    print(f"step {idx}: loss {loss.item():.4f}")

 if __name__ == "__main__":
   parser = argparse.ArgumentParser()
```

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
