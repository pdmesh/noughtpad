"""Generate the sample notebook in samples/vault (run once; output is committed)."""
import os, sys, random, re
from datetime import datetime

ROOT = sys.argv[1]
B32 = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"
rng = random.Random(7)

def ulid(dt):
    ms = int(dt.timestamp() * 1000)
    t = "".join(B32[(ms >> (5 * i)) & 31] for i in reversed(range(10)))
    return t + "".join(rng.choice(B32) for _ in range(16))

def slug(s):
    words = re.findall(r"[a-z0-9]+", s.lower().replace("'", ""))[:6]
    return "-".join(words) or "note"

def q(s):
    return f'"{s}"' if re.search(r"[:#,\[\]{}]|^\d", s) else s

# (datetime, labels, source_title, source_url, title, body)
N = []
def n(when, body, labels=(), source=None, url=None, title=None):
    N.append((datetime.fromisoformat(when + "+05:30"), list(labels), source, url, title, body.strip() + "\n"))

# --- VC: a thesis that evolves over the year -----------------------------
n("2026-01-14T09:10", """Read about VC power-law returns. A handful of outlier companies return the entire fund; most investments return little or nothing.

> The best investment in a successful fund equals or outperforms the entire rest of the fund combined.""", ["VC", "Books"], "Venture Deals")
n("2026-01-20T22:05", "If returns follow a power law, the job is not to avoid losses but to make sure you are in the outliers at all.", ["VC"])
n("2026-02-03T07:40", "A fund can have many mediocre investments and still perform well if one company becomes enormous. Does that mean diligence on downside risk is mostly wasted effort?", ["VC"])
n("2026-03-03T22:40", """Ownership may matter more than I expected.

If one company returns the fund, entry ownership decides *how much* it returns. 2% of a $10B outcome is $200M; 10% is $1B.""", ["VC"])
n("2026-03-18T21:15", """Quick maths on fund returns. For a fund of size $F$ to return $3\\times$, with ownership $o$ at exit, the winners need a combined exit value of

$$
V = \\frac{3F}{o}
$$

A $100M fund at 5% ownership needs $6B of exits.""", ["VC"], title="Fund return maths")
n("2026-04-09T08:30", "Portfolio construction: reserves for follow-ons, number of initial checks, concentration versus diversification. Most funds reserve 40–60% for follow-ons.", ["VC", "Books"], "Secrets of Sand Hill Road")
n("2026-05-22T19:00", "Talked to an angel friend: she thinks ownership targets make funds pass on the best rounds because they are too competitive to get meaningful ownership. Tension with my March note.", ["VC"])
n("2026-06-20T08:05", "Read about portfolio construction again: concentration only works if you can pick. If you can't, diversification buys you more shots at an outlier.", ["VC"], "Venture Capital Journal", "https://example.com/vc-portfolio")
n("2026-09-12T23:10", "Ownership seems to matter more than I initially thought in VC. Need to understand how this interacts with fund construction.", ["VC", "Product"])
n("2026-10-02T07:30", """## Is "good company" the right VC framework?

The question is not just whether a company is good, but whether the investment can produce a meaningful fund outcome.

- [ ] compare with portfolio maths
- [x] reread power-law note
- [ ] ask two GPs how they decide ownership targets""", ["VC"])

# --- Product / customer discovery ---------------------------------------
n("2026-02-11T10:00", """Customers often describe what they want rather than what they actually do.

> Talk about their life instead of your idea. Ask about specifics in the past instead of generics or opinions about the future.

This seems relevant to how I should interview users for Decaframe.""", ["Product", "Books"], "The Mom Test")
n("2026-02-12T08:20", "Compliments are not data. Every \"that sounds great\" in last week's calls was worthless.", ["Product"], "The Mom Test")
n("2026-04-28T17:45", "User interviews for the notes idea: four of five people complained about folders and tagging. Nobody mentioned search. Everybody had a graveyard of notes they never reopened.", ["Product"])
n("2026-05-03T09:00", "Onboarding should get someone writing in under ten seconds. No setup screens, no templates, no choosing a folder structure.", ["Product"])
n("2026-07-15T21:30", "What job is a notes app hired for? Maybe not \"remember things\" but \"make me feel my reading and thinking adds up to something\".", ["Product"])

# --- Decaframe (presentation software) -----------------------------------
n("2026-03-25T11:10", "Presentation software should let slides flow like documents, with layouts chosen by what the content says.", ["Decaframe"])
n("2026-06-02T16:00", "Idea for organising slide templates by intent (compare, sequence, quote, metric) rather than by visual style.", ["Decaframe"])
n("2026-09-12T23:25", "Had an idea for a better way to organise presentation software notes: one running document per deck instead of scattered files.", ["Decaframe"])
n("2026-08-08T14:20", """Rough architecture for the deck engine:

```mermaid
graph LR
  Content[Content blocks] --> Layout[Layout chooser]
  Layout --> Theme[Theme tokens]
  Theme --> Render[Renderer]
  Render --> Export[PDF / PPTX]
```

Layout should never need a coordinate from the author.""", ["Decaframe", "Software Architecture"], title="Deck engine architecture")

# --- Knowledge system (this product) ---------------------------------------
n("2026-05-30T22:50", "The knowledge system idea seems to be moving from \"AI-organised notes\" toward \"a blank notebook with background intelligence\".", ["Product"])
n("2026-08-21T07:15", "Zettelkasten is a good underlying model but a bad workflow to impose. Atomic notes are a cost the writer shouldn't pay.", ["Product", "Books"], "How to Take Smart Notes")
n("2026-10-19T20:40", "What if the value isn't connecting notes but showing how a thought changed over time? The relationship has a history.", ["Product"])

# --- Software architecture ------------------------------------------------
n("2026-04-14T13:00", "Local-first: the files are the source of truth; any database is a cache you can delete. Makes sync someone else's problem (iCloud, git, Syncthing).", ["Software Architecture"])
n("2026-07-01T10:30", "SQLite FTS5 is absurdly good for personal-scale search. BM25 ranking, prefix queries, highlighting, all in one file.", ["Software Architecture"])

# --- Travel -----------------------------------------------------------------
n("2026-02-25T20:00", "Tokyo itinerary draft: three days — Shibuya and Harajuku, Asakusa and Ueno, day trip to Nikko.", ["Travel"])
n("2026-03-01T19:30", "Kyoto: book a ryokan near Gion. Visit Fushimi Inari before 7am to avoid crowds; Arashiyama bamboo grove the same morning is too much.", ["Travel"])
n("2026-09-12T23:40", "Need to revisit the Tokyo and Kyoto itinerary — the dates moved to April, cherry blossom season.", ["Travel"])

# --- Cooking ------------------------------------------------------------------
n("2026-01-26T18:10", "Sourdough starter needs feeding twice a day at room temperature. Today's dough was too slack — probably over-hydrated.", ["Cooking"])
n("2026-02-08T12:00", "Bread hydration around 70% made a much better crumb than the last loaf. Longer cold proof overnight helped too.", ["Cooking"])

n("2026-03-14T09:30", "Third loaf: autolyse for an hour before adding the starter. Much better oven spring and an open crumb.", ["Cooking"])

# --- Health: a new thread in the last few weeks ("ideas forming") -------------
n("2026-10-12T07:00", "Started training for a half marathon in February. Three runs a week, one long run on Sunday.", ["Health"])
n("2026-10-21T06:45", "Long run of 14 km felt easy at a slow pace. Easy runs really should be easy.", ["Health"])
n("2026-10-30T07:10", "Half marathon plan: build the long run to 18 km by January, then taper for two weeks before the race.", ["Health"])

# --- Personal, unlabelled -----------------------------------------------------
n("2026-04-02T06:50", "Woke up thinking about how much of my reading never turns into anything. Not because it was bad, but because nothing brought it back.")
n("2026-06-11T23:30", "Decision: no more than two side projects at a time. Writing this down so future me can disagree with it properly.")
n("2026-10-27T08:00", "Open question: do I actually believe concentration beats diversification, or do I just like the story?")

os.makedirs(ROOT, exist_ok=True)
for when, labels, source, url, title, body in N:
    lines = ["---", f"id: {ulid(when)}"]
    if title:
        lines.append(f"title: {q(title)}")
    lines.append(f"created: {when.isoformat()}")
    if labels:
        lines.append("labels: [" + ", ".join(q(l) for l in labels) + "]")
    if source or url:
        lines.append("source:")
        if source: lines.append(f"  title: {q(source)}")
        if url: lines.append(f"  url: {url}")
    lines.append("---")
    first = title or next(l for l in body.splitlines() if l.strip())
    first = first.lstrip("#> ").strip()
    rel = f"{when:%Y/%m}/{when:%Y-%m-%d-%H%M}-{slug(first)}.md"
    path = os.path.join(ROOT, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        f.write("\n".join(lines) + "\n\n" + body)

# A plain Markdown file with no frontmatter, as another app would write it.
os.makedirs(os.path.join(ROOT, "imported"), exist_ok=True)
with open(os.path.join(ROOT, "imported", "2026-05-15 reading list.md"), "w") as f:
    f.write("# Reading list\n\n- *The Mom Test* — Rob Fitzpatrick\n- *Secrets of Sand Hill Road* — Scott Kupor\n- *How to Take Smart Notes* — Sönke Ahrens\n")
print(len(N) + 1, "notes")
