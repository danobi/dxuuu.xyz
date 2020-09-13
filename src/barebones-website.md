% Setting up a barebones website

I had a very specific workflow in mind when I set out to host a website. First,
I had no desire to be locked into a specific platform. I've experimented with
Github pages but it's pretty locked into Github's ecosystem.

Second, publication and hosting had to be as cross platform as possible. I
wanted to be able to write and publish from anywhere.

Lastly, I wanted writing to be as hassle-free and minimally invasive as
possible. This meant I did not want to figure out how
[Jekyll](https://jekyllrb.com/) or [Hugo](https://gohugo.io/) or whatever-
have-you works. I understand that those tools exist for specific reasons,
but all I wanted was a simple setup.

In other words, I wanted something that would be simple, straightforward, and
portable.

I came up with the following workflow:

1. I write a post or make a change to my website. I do this using Pandoc
   flavored markdown.

2. I push the commit to my site's Github repo. Note: I could use any git
   hosting service here, including my own.

3. Then on my server a systemd timer eventually fires. It pulls down the most
   recent commits and compiles all the Markdown files to HTML. It then updates
   my webserver's hosting directory.

4. Some unfortunate soul visits my website and my webserver serves them this
   junk.

Take a peek at all the [backend configuration
here](https://github.com/danobi/dxuuu.xyz).

### Notes

* Yes, I hard code links to other pages on my site. It might be a bad idea.

* Lots of inspiration was drawn from [Dan Luu](https://danluu.com/)'s website'.
