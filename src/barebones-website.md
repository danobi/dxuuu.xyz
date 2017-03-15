% Setting up a barebones website

I had a very specific workflow in mind when I set out to host a website. First, I had no desire
to be locked into a specific platform. I'd experimented with Github pages in the past, but it was
inexorably tied to Github's repository system.

Second, publication and hosting had to be as cross platform as possible. I wanted to be able to
publish from my Windows laptop. I wanted to be able to publish from my Linux desktop. I wanted to
be able to publish on a 56k connection in Antarctica on a $50 netbook.

Lastly, I wanted writing to be as hassle-free and minimally invasive as possible. This meant I did
not want to figure out how [Jekyll](https://jekyllrb.com/) or [Hugo](https://gohugo.io/) or whatever-
have-you. Now, I understand that those tools exist for specific reasons, but all I wanted was (say
it with me) **a barebones website**. 

In other words, I wanted something that would be simple, straightforward, and portable.

Luckily for me, people have been using computers for a while now and have created tools that allow
lazy bums like me to finesse brain dead solutions. To spare you the theatrics, this is how my website
works:

1. I write a post or make a change to my website. I do this using Pandoc flavored markdown.

2. I push the commit to my site's Github repo. *Note, I could use any git hosting service here, including
my own*.

3. A systemd timer eventually fires and compiles all my Markdown files to HTML. Then it updates my
webserver's hosting directory.

4. Some unfortunate soul visits my website and my webserver serves them this junk.

So you might be wondering, "How does this guy do anything fancy with HTML or CSS or Javascript?".
The answer is: I don't. This is a barebones website.

Take a peek at all the [backend configuration here](https://github.com/danobi/dxuuu.xyz).

### Notes

* Yes, I hard code links to other pages on my site. I figure this is fine because I'm not Leo
Tolstoy or something and write mountains of material. 

* I'd be lying bastard if I didn't draw inspiration for this site from [Dan Luu](https://danluu.com/).
His writing is pretty good and you should read some.
