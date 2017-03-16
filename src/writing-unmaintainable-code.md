% On writing unmaintainable code

The internet's beaten to death the concept of [legacy code][1], [unmaintainable code][2], [bad code][3],
and <insert-word-here>-code. It's safe to say nobody really likes dealing with code that doesn't make
much sense. But how about the other side of the coin?

Consider this: you're tasked with writing inherently tricky code. You know this code will be nigh unreadable
next week, but you also know someone has to write it. How do you manage future pain of dealing with this code?

Last week I gave that question some thought and I now believe there are two options in such a scenario.

1. Write a lot of comments to help the next guy that comes along to understand your genius
  (or lack thereof).

2. Explain what this snippet of code does with clear {pre,post}-conditions so it can simply
  be ripped out and replaced next time someone needs it replaced.

Option 1 is generally good advice. Comments enhance readability after all. But consider this snippet:

``` {#function .c++ .numberLines startFrom="1"}
/* Replace a subsection of a buffer with a replacement string */
void
Scrubber::scrub_buffer(char *buffer, Scrub *scrub) const
{
  char *buffer_ptr;
  int num_matched;
  int match_len;
  int replacement_len;
  int buffer_len = strlen(buffer);

  buffer_ptr      = buffer;
  match_len       = scrub->ovector[1] - scrub->ovector[0];
  replacement_len = scrub->replacement.size();

  if (replacement_len <= match_len) {
    buffer_ptr += scrub->ovector[0];
    memcpy(buffer_ptr, scrub->replacement.ptr(), replacement_len);
    buffer_ptr += replacement_len;
    memmove(buffer_ptr, buffer + scrub->ovector[1], buffer_len - scrub->ovector[1] + 1);
    ink_assert(buffer[buffer_len - (match_len - replacement_len)] == '\0');
  } else {
    int n_slide = buffer_len - scrub->ovector[0] - replacement_len;
    if (n_slide < 0) {
      replacement_len += n_slide;
    } else {
      buffer_ptr += scrub->ovector[0] + replacement_len;
      memmove(buffer_ptr, buffer + scrub->ovector[1], n_slide);
    }
    buffer_ptr = buffer + scrub->ovector[0];
    memcpy(buffer_ptr, scrub->replacement.ptr(), replacement_len);
    buffer[buffer_len] = '\0';
  }
}
```

Notice all the C-style pointer fiddling. Reasoning through the code itself would take less time
than reading a sufficiently detailed enough comment for each line to elucidate each statement.
Thus, writing sufficient comments would theoretically add complexity to the codebase.

Now consider option 2. To continue the previous code example, take a look at this illustration:

``` {#illustration .c++ .numberLines startFrom="1"}
/*
 * When scrubbing the buffer in place, there are 2 scenarios we need to consider:
 *
 *   1) The replacement text length is shorter or equal to the text we want to scrub away
 *   2) The replacement text is longer than the text we want to scrub away
 *
 * In case 1, we simply "slide" everything left a bit. Our final buffer should
 * look like this (where XXXX is the replacment text):
 *
 *                                new_end  orig_end
 *                                    V      V
 *   -----------------------------------------
 *   |ORIGINAL TEXT|XXXX|ORIGINAL TEXT|      |
 *   -----------------------------------------
 *
 * In case 2, since the final buffer would be longer than the original allocated buffer,
 * we need to truncate everything that would have run over the original end of the buffer.
 * The final buffer should look like this:
 *
 *                                         new_end
 *                                        orig_end
 *                                           V
 *   -----------------------------------------
 *   |ORIGINAL TEXT|XXXXXXXXXXXXXXXXXXX|ORIGI|NAL TEXT
 *   -----------------------------------------
 *
 */
```

To quote an old english idiom, "A picture is worth a thousand words". In this case, our picture
is actually some ASCII art. (If you don't believe ASCII can be art you should go to a museum of modern
art). Now, the next person who needs to change this function has two options of his own:

1. Understand and modify the existing code with knowledge of the scope of the code.

2. Rip out all the old code and write something (hopefully) better.

Further examination of those two options are out of the scope of this post :).


[1]: https://news.ycombinator.com/item?id=13911553
[2]: https://www.doc.ic.ac.uk/%7Esusan/475/unmain.html
[3]: http://higherorderlogic.com/2010/07/bad-code-isnt-technical-debt-its-an-unhedged-call-option/
