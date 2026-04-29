linux: CC=gcc
win: CC=x86_64-w64-mingw32-gcc

SDIR=src
IDIR=inc
LDIR=lib
ODIR=bin

TARGETS=binarize segment sbbump sbcockpit sbeffect sbengine sbhitbox sbmodel \
	sbmotion sbshader sbsound sbstage sbterrain sbtext sbtexture sbweapon \
	sblsq

CFLAGS=-g -lm -I$(IDIR)

.PHONY: all linux win clean
all: linux win
linux: $(ODIR) $(patsubst %,$(ODIR)/%,$(TARGETS))
win: $(ODIR) $(patsubst %,$(ODIR)/%.exe,$(TARGETS))
clean:
	rm -rf bin/

$(ODIR)/binarize $(ODIR)/binarize.exe: $(SDIR)/binarize.c
$(ODIR)/segment $(ODIR)/segment.exe: $(SDIR)/segment.c $(LDIR)/sha1.c

$(ODIR)/sbbump $(ODIR)/sbbump.exe: $(SDIR)/sbbump.c
$(ODIR)/sbcockpit $(ODIR)/sbcockpit.exe: $(SDIR)/sbcockpit.c $(LDIR)/jWrite.c
$(ODIR)/sbeffect $(ODIR)/sbeffect.exe: $(SDIR)/sbeffect.c $(LDIR)/jWrite.c
$(ODIR)/sbengine $(ODIR)/sbengine.exe: $(SDIR)/sbengine.c $(LDIR)/jWrite.c
$(ODIR)/sbhitbox $(ODIR)/sbhitbox.exe: $(SDIR)/sbhitbox.c
$(ODIR)/sblsq $(ODIR)/sblsq.exe: $(SDIR)/sblsq.c $(LDIR)/jWrite.c
$(ODIR)/sbmodel $(ODIR)/sbmodel.exe: $(SDIR)/sbmodel.c
$(ODIR)/sbmotion $(ODIR)/sbmotion.exe: $(SDIR)/sbmotion.c
$(ODIR)/sbshader $(ODIR)/sbshader.exe: $(SDIR)/sbshader.c
$(ODIR)/sbsound $(ODIR)/sbsound.exe: $(SDIR)/sbsound.c $(LDIR)/jWrite.c
$(ODIR)/sbstage $(ODIR)/sbstage.exe: $(SDIR)/sbstage.c $(LDIR)/jWrite.c
$(ODIR)/sbterrain $(ODIR)/sbterrain.exe: $(SDIR)/sbterrain.c $(LDIR)/swizzle.c
$(ODIR)/sbtext $(ODIR)/sbtext.exe: $(SDIR)/sbtext.c
$(ODIR)/sbtexture $(ODIR)/sbtexture.exe: $(SDIR)/sbtexture.c $(LDIR)/swizzle.c
$(ODIR)/sbweapon $(ODIR)/sbweapon.exe: $(SDIR)/sbweapon.c $(LDIR)/jWrite.c

$(ODIR):
	mkdir $(ODIR)

$(patsubst %,$(ODIR)/%,$(TARGETS)) $(patsubst %,$(ODIR)/%.exe,$(TARGETS)):
	$(CC) -o $@ $^ $(CFLAGS)
