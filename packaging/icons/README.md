# Latexo icon

`latexo.svg` is the master: the pulse line on a dark rounded square of the design (the side bar
logo of the Artifact). It is the same file as `crates/latexo-app/assets/latexo.svg`, which the side
bar paints.

The PNG sizes under `png/` and the Windows icon are generated from it and committed:

```
for s in 16 24 32 48 64 128 256 512 1024; do
  rsvg-convert -w $s -h $s latexo.svg -o png/latexo-$s.png
done
magick png/latexo-{16,24,32,48,64,128,256}.png ../../crates/latexo-app/assets/latexo.ico
```
