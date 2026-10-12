#!/usr/bin/env python3
"""Makes the biome music: short loopable tracks, written to assets/music/.

    python3 tools/make_music.py              all of them
    python3 tools/make_music.py desert_1     just the ones named

Every track is a handful of bars of simple synth voices (lead, bass, pad, a little drum
kit), written to be played in a loop. Notes that ring past the end of a track wrap round to
the start, so the loop has no seam. Plain Python, nothing to install. The same seeds give
the same tracks every time; change a seed or a setting below to get a different tune.
"""
import math
import os
import random
import struct
import wave

RATE = 16000
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "music")

# Scales, as semitones above the root.
MAJOR = [0, 2, 4, 5, 7, 9, 11]
MAJOR_PENT = [0, 2, 4, 7, 9]
MINOR = [0, 2, 3, 5, 7, 8, 10]
MINOR_PENT = [0, 3, 5, 7, 10]
DORIAN = [0, 2, 3, 5, 7, 9, 10]
MIXOLYDIAN = [0, 2, 4, 5, 7, 9, 10]
LYDIAN = [0, 2, 4, 6, 7, 9, 11]
PHRYGIAN = [0, 1, 3, 5, 7, 8, 10]
HIJAZ = [0, 1, 4, 5, 7, 8, 10]
LOCRIAN = [0, 1, 3, 5, 6, 8, 10]
HARMONIC_MINOR = [0, 2, 3, 5, 7, 8, 11]


def hz(midi):
    return 440.0 * 2 ** ((midi - 69) / 12)


# ---- voices: each returns a function of (phase in cycles, seconds into the note) ----

def sine(p, t):
    return math.sin(2 * math.pi * p)


def triangle(p, t):
    return 4 * abs(p % 1.0 - 0.5) - 1


def square(p, t):
    return 1.0 if p % 1.0 < 0.5 else -1.0


def pulse(p, t):
    return 1.0 if p % 1.0 < 0.25 else -1.0


def saw(p, t):
    return 2 * (p % 1.0) - 1


def bell(p, t):
    # A sine with a quieter, inharmonic partner: a soft bell or music box.
    return 0.8 * math.sin(2 * math.pi * p) + 0.35 * math.sin(2 * math.pi * p * 2.76) * math.exp(-t * 5)


def flute(p, t):
    vibrato = 0.004 * math.sin(2 * math.pi * 5.0 * t) * min(1.0, t * 3)
    return 0.9 * math.sin(2 * math.pi * (p + vibrato)) + 0.15 * math.sin(4 * math.pi * p)


def pluck(p, t):
    # Bright at first, dull a moment later.
    return math.sin(2 * math.pi * p) + math.exp(-t * 9) * 0.6 * square(p * 2, t)


VOICES = {
    "sine": sine, "triangle": triangle, "square": square, "pulse": pulse, "saw": saw,
    "bell": bell, "flute": flute, "pluck": pluck,
}


class Track:
    def __init__(self, name, seed, bpm, bars, root, scale, progression, lead="triangle",
                 bass="triangle", pad="sine", lead_gain=0.5, bass_gain=0.5, pad_gain=0.28,
                 density=0.7, octave=0, drums=None, arp=None, attack=0.01, release=0.25,
                 swing=0.0, drone=False):
        self.name, self.bpm, self.bars, self.root, self.scale = name, bpm, bars, root, scale
        self.progression, self.lead, self.bass, self.pad = progression, lead, bass, pad
        self.lead_gain, self.bass_gain, self.pad_gain = lead_gain, bass_gain, pad_gain
        self.density, self.octave, self.drums, self.arp = density, octave, drums, arp
        self.attack, self.release, self.swing, self.drone = attack, release, swing, drone
        self.rng = random.Random(seed)
        self.beat = 60.0 / bpm
        self.frames = int(round(bars * 4 * self.beat * RATE))
        self.buf = [0.0] * self.frames

    # -- low level --

    def add(self, start_beat, length_beats, midi, voice, gain, attack=None, release=None):
        """Mixes one note in, wrapping round the end of the loop."""
        attack = self.attack if attack is None else attack
        release = self.release if release is None else release
        start = int(start_beat * self.beat * RATE)
        body = length_beats * self.beat
        total = body + release
        n = int(total * RATE)
        freq = hz(midi)
        fn = VOICES[voice]
        for i in range(n):
            t = i / RATE
            env = min(1.0, t / max(attack, 1e-4))
            if t > body:
                env *= max(0.0, 1.0 - (t - body) / release)
            # A gentle decay so held notes don't sit flat.
            env *= 0.55 + 0.45 * math.exp(-t * 1.5)
            self.buf[(start + i) % self.frames] += fn(freq * t, t) * env * gain

    def noise_hit(self, start_beat, length, gain, low=False):
        start = int(start_beat * self.beat * RATE)
        n = int(length * RATE)
        last = 0.0
        for i in range(n):
            t = i / RATE
            env = math.exp(-t * (6.0 / length))
            x = self.rng.uniform(-1, 1)
            if low:
                last += (x - last) * 0.25  # dull
                x = last
            self.buf[(start + i) % self.frames] += x * env * gain

    def kick(self, start_beat, gain=0.7):
        start = int(start_beat * self.beat * RATE)
        n = int(0.22 * RATE)
        phase = 0.0
        for i in range(n):
            t = i / RATE
            f = 40 + 90 * math.exp(-t * 28)
            phase += f / RATE
            self.buf[(start + i) % self.frames] += math.sin(2 * math.pi * phase) * math.exp(-t * 14) * gain

    # -- music --

    def degree_midi(self, degree):
        """MIDI note for a scale degree (0 is the root; it can go negative or past the octave)."""
        n = len(self.scale)
        octave, index = divmod(degree, n)
        return self.root + 12 * octave + self.scale[index]

    def build(self):
        rng = self.rng
        prog = self.progression
        for bar in range(self.bars):
            chord = prog[bar % len(prog)]
            base = bar * 4
            # Pad: the chord, held for the bar.
            for k in (0, 2, 4):
                self.add(base, 4.0, self.degree_midi(chord + k) + 12, self.pad, self.pad_gain / 3,
                         attack=0.5, release=0.6)
            # Bass: root, with a fifth or an octave for movement.
            bass_pattern = rng.choice([[0, 0, 4, 0], [0, 2, 0, 4], [0, 0, 0, 0], [0, 4, 0, 2]])
            if self.drone:
                self.add(base, 4.0, self.degree_midi(chord) - 24, self.bass, self.bass_gain,
                         attack=0.3, release=0.8)
            else:
                for beat, step in enumerate(bass_pattern):
                    self.add(base + beat, 0.9, self.degree_midi(chord + step) - 12, self.bass,
                             self.bass_gain, release=0.15)
            # Optional arpeggio of the chord in eighth notes.
            if self.arp:
                order = [0, 2, 4, 2] if self.arp == "up" else [4, 2, 0, 2]
                for i in range(8):
                    d = chord + order[i % 4] + (7 if i % 8 >= 4 else 0)
                    self.add(base + i * 0.5, 0.45, self.degree_midi(d) + 12 * self.octave,
                             "bell", self.lead_gain * 0.55, release=0.3)
            # Lead: a walk over the scale that leans on chord tones on the beat.
            self.lead_bar(bar, base, chord)
            # Drums.
            if self.drums:
                self.drum_bar(base, bar)

    def lead_bar(self, bar, base, chord):
        rng = self.rng
        last = bar == self.bars - 1
        position = self.state if hasattr(self, "state") else chord + 2
        step = 0.0
        slots = []
        while step < 4.0:
            length = rng.choice([0.5, 0.5, 1.0, 1.0, 1.5, 2.0])
            length = min(length, 4.0 - step)
            slots.append((step, length))
            step += length
        for i, (at, length) in enumerate(slots):
            if rng.random() > self.density and not (at == 0 and bar % 2 == 0):
                continue  # a rest
            on_beat = at == int(at)
            if last and i == len(slots) - 1:
                position = chord  # come home at the end of the loop
            elif on_beat and rng.random() < 0.6:
                tones = [chord, chord + 2, chord + 4, chord + 7]
                position = min(tones, key=lambda d: abs(d - position) + rng.random() * 1.5)
            else:
                position += rng.choice([-2, -1, -1, 1, 1, 2])
            position = max(chord - 3, min(chord + 9, position))
            swing = self.swing if (at * 2) % 2 == 1 else 0.0
            self.add(base + at + swing, length * 0.95,
                     self.degree_midi(position) + 12 * self.octave + 12, self.lead,
                     self.lead_gain, release=self.release)
        self.state = position

    def drum_bar(self, base, bar):
        kind = self.drums
        if kind == "soft":
            self.kick(base, 0.35)
            self.kick(base + 2, 0.3)
            self.noise_hit(base + 1, 0.08, 0.06, low=True)
            self.noise_hit(base + 3, 0.08, 0.06, low=True)
        elif kind == "march":
            for b in range(4):
                self.kick(base + b, 0.55 if b % 2 == 0 else 0.3)
            for b in (1, 3):
                self.noise_hit(base + b, 0.12, 0.16)
        elif kind == "hand":
            # A hand drum: low, low, high, low, high.
            for b, low in [(0, True), (0.75, False), (1.5, True), (2.5, False), (3, True)]:
                self.noise_hit(base + b, 0.1, 0.22 if low else 0.12, low=low)
            self.kick(base, 0.3)
        elif kind == "heartbeat":
            self.kick(base, 0.7)
            self.kick(base + 0.5, 0.45)
            if bar % 2 == 1:
                self.kick(base + 2, 0.6)
                self.kick(base + 2.5, 0.4)
        elif kind == "tick":
            for b in range(8):
                self.noise_hit(base + b * 0.5, 0.03, 0.05)
        elif kind == "stomp":
            self.kick(base, 0.6)
            self.kick(base + 1.5, 0.4)
            self.kick(base + 2, 0.6)
            self.noise_hit(base + 1, 0.1, 0.14)
            self.noise_hit(base + 3, 0.1, 0.14)

    def write(self):
        peak = max(abs(x) for x in self.buf) or 1.0
        scale = 0.8 / peak
        os.makedirs(OUT, exist_ok=True)
        path = os.path.join(OUT, self.name + ".wav")
        with wave.open(path, "wb") as w:
            w.setnchannels(1)
            w.setsampwidth(2)
            w.setframerate(RATE)
            w.writeframes(b"".join(struct.pack("<h", int(x * scale * 32767)) for x in self.buf))
        print(f"{self.name}: {self.frames / RATE:.1f}s, {os.path.getsize(path) // 1024} KB")


# Chord progressions, as scale degrees of the root (0 = I, 3 = IV, 4 = V, 5 = vi ...).
TRACKS = [
    # Plains: the original tune (assets/looping_tune.wav) plays here too. Two more bright ones.
    dict(name="plains_1", seed=111, bpm=96, bars=8, root=60, scale=MAJOR_PENT,
         progression=[0, 2, 3, 1], lead="triangle", lead_gain=0.5, bass="triangle", pad="sine",
         density=0.7, drums="soft", release=0.3),
    dict(name="plains_2", seed=12, bpm=88, bars=8, root=55, scale=MAJOR,
         progression=[0, 5, 3, 4], lead="flute", lead_gain=0.5, pad="sine", density=0.65,
         drums=None, release=0.3),
    # Meadow: music box and gentle arpeggios.
    dict(name="meadow_1", seed=21, bpm=92, bars=8, root=64, scale=MAJOR,
         progression=[0, 3, 4, 0], lead="bell", lead_gain=0.45, arp="up", density=0.55,
         pad="sine", release=0.4),
    dict(name="meadow_2", seed=222, bpm=108, bars=8, root=60, scale=MAJOR,
         progression=[0, 4, 5, 3], lead="triangle", lead_gain=0.5, bass="triangle", pad="sine",
         density=0.8, drums="soft", release=0.2, swing=0.05),
    dict(name="meadow_3", seed=23, bpm=76, bars=8, root=67, scale=LYDIAN,
         progression=[0, 1, 0, 4], lead="flute", lead_gain=0.42, arp="down", density=0.5,
         pad="sine", release=0.4),
    # Forest: woody, a little mysterious.
    dict(name="forest_1", seed=31, bpm=78, bars=8, root=57, scale=DORIAN,
         progression=[0, 3, 0, 4], lead="flute", lead_gain=0.5, bass="triangle", pad="triangle",
         density=0.6, drums="soft", release=0.35),
    dict(name="forest_2", seed=232, bpm=100, bars=8, root=55, scale=MAJOR,
         progression=[0, 3, 0, 4], lead="pulse", lead_gain=0.2, bass="triangle", pad="triangle",
         density=0.8, drums="soft", release=0.15, swing=0.1),
    dict(name="forest_3", seed=33, bpm=66, bars=8, root=59, scale=MAJOR,
         progression=[0, 4, 5, 3], lead="flute", lead_gain=0.45, density=0.5,
         pad="triangle", release=0.5),
    # Swamp: slow, low and eerie. Only soft voices (no saw or square waves, no noise drums).
    dict(name="swamp_1", seed=141, bpm=58, bars=8, root=45, scale=PHRYGIAN,
         progression=[0, 0, 1, 0], lead="bell", lead_gain=0.38, bass="sine", bass_gain=0.5,
         pad="triangle", pad_gain=0.3, density=0.3, drone=True, drums="heartbeat",
         release=0.8, octave=-1),
    dict(name="swamp_2", seed=142, bpm=62, bars=8, root=47, scale=MINOR,
         progression=[0, 5, 0, 4], lead="flute", lead_gain=0.45, bass="triangle", pad="sine",
         density=0.4, release=0.6, octave=-1),
    dict(name="swamp_3", seed=143, bpm=52, bars=8, root=43, scale=LOCRIAN,
         progression=[0, 1, 0, 2], lead="bell", lead_gain=0.35, bass="sine", bass_gain=0.5,
         pad="sine", pad_gain=0.34, density=0.25, drone=True, release=0.9, octave=-1),
    # Desert: a snake-charmer scale, plucked strings and a hand drum.
    dict(name="desert_1", seed=151, bpm=66, bars=8, root=57, scale=HARMONIC_MINOR,
         progression=[0, 5, 0, 4], lead="bell", lead_gain=0.4, bass="triangle", pad="sine",
         density=0.45, drums="soft", arp="down", release=0.6, octave=0),
    dict(name="desert_2", seed=352, bpm=60, bars=8, root=50, scale=MAJOR_PENT,
         progression=[0, 3, 0, 1], lead="bell", lead_gain=0.38, bass="sine", bass_gain=0.4,
         pad="triangle", pad_gain=0.4, density=0.3, drone=True, release=0.9, attack=0.05),
    dict(name="desert_3", seed=53, bpm=96, bars=8, root=64, scale=HIJAZ,
         progression=[0, 3, 0, 4], lead="square", lead_gain=0.2, bass="triangle", pad="triangle",
         density=0.8, drums="march", arp="up", release=0.15),
    # Badlands: dusty and wide, with a steady stomp.
    dict(name="badlands_1", seed=61, bpm=92, bars=8, root=50, scale=MIXOLYDIAN,
         progression=[0, 3, 0, 4], lead="pluck", lead_gain=0.42, bass="triangle", pad="triangle",
         density=0.65, drums="stomp", release=0.25),
    dict(name="badlands_2", seed=62, bpm=70, bars=8, root=48, scale=MINOR_PENT,
         progression=[0, 2, 3, 4], lead="flute", lead_gain=0.45, pad="sine", density=0.5,
         drums="stomp", release=0.4),
    dict(name="badlands_3", seed=63, bpm=82, bars=8, root=55, scale=DORIAN,
         progression=[0, 0, 3, 1], lead="saw", lead_gain=0.16, bass="triangle", pad="triangle",
         density=0.6, drums="march", release=0.25),
    # Volcanic (ashlands): the original scary loop (assets/scary_loop.wav) plays here too.
    # Two more, dark and heavy but made of soft voices only.
    dict(name="volcanic_1", seed=171, bpm=68, bars=8, root=38, scale=HARMONIC_MINOR,
         progression=[0, 5, 0, 4], lead="flute", lead_gain=0.45, bass="triangle", bass_gain=0.55,
         pad="triangle", pad_gain=0.3, density=0.4, drums="heartbeat", release=0.5, octave=-1),
    dict(name="volcanic_2", seed=172, bpm=60, bars=8, root=36, scale=LOCRIAN,
         progression=[0, 0, 1, 0], lead="bell", lead_gain=0.38, bass="sine", bass_gain=0.5,
         pad="triangle", pad_gain=0.34, density=0.3, drone=True, drums="heartbeat",
         release=0.8, octave=-1),
    # Snow: sparse, high and bright, with long shimmering pads.
    dict(name="snow_1", seed=81, bpm=64, bars=8, root=65, scale=LYDIAN,
         progression=[0, 4, 5, 1], lead="bell", lead_gain=0.45, bass="sine", pad="sine",
         density=0.4, release=0.8),
    dict(name="snow_2", seed=82, bpm=72, bars=8, root=62, scale=MAJOR_PENT,
         progression=[0, 3, 1, 4], lead="bell", lead_gain=0.4, arp="up", pad="triangle",
         density=0.45, drums="tick", release=0.6),
    dict(name="snow_3", seed=83, bpm=56, bars=8, root=57, scale=MINOR,
         progression=[0, 5, 3, 4], lead="flute", lead_gain=0.4, bass="sine", pad="sine",
         density=0.35, release=0.8),
]


def main():
    import sys
    # With names on the command line, only those tracks are made: make_music.py desert_1
    wanted = set(sys.argv[1:])
    for spec in TRACKS:
        if wanted and spec["name"] not in wanted:
            continue
        track = Track(**spec)
        track.build()
        track.write()


if __name__ == "__main__":
    main()
