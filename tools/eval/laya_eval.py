"""Hand-labelled pair benchmark for Laya (scratch tool)."""
import sys, time, warnings
warnings.filterwarnings("ignore")
from laya import Router

N = {
 "pl": "Read about VC power-law returns. A handful of outlier companies return the entire fund; most investments return little or nothing.",
 "job": "If returns follow a power law, the job is not to avoid losses but to make sure you are in the outliers at all.",
 "own": "Ownership may matter more than I expected. If one company returns the fund, entry ownership decides how much it returns.",
 "angel": "Talked to an angel friend: she thinks ownership targets make funds pass on the best rounds because they are too competitive to get meaningful ownership. Tension with my March note.",
 "own2": "Ownership seems to matter more than I initially thought in VC. Need to understand how this interacts with fund construction.",
 "conc": "Read about portfolio construction again: concentration only works if you can pick. If you can't, diversification buys you more shots at an outlier.",
 "q": "Open question: do I actually believe concentration beats diversification, or do I just like the story?",
 "mom": "Customers often describe what they want rather than what they actually do. Talk about their life instead of your idea.",
 "ui": "User interviews for the notes idea: four of five people complained about folders and tagging. Nobody mentioned search.",
 "job2": "What job is a notes app hired for? Maybe not remember things but make me feel my reading and thinking adds up to something.",
 "sour": "Sourdough starter needs feeding twice a day at room temperature. Today's dough was too slack, probably over-hydrated.",
 "loaf": "Third loaf: autolyse for an hour before adding the starter. Much better oven spring and an open crumb.",
 "tok": "Tokyo itinerary draft: three days: Shibuya and Harajuku, Asakusa and Ueno, day trip to Nikko.",
 "kyo": "Kyoto: book a ryokan near Gion. Visit Fushimi Inari before 7am to avoid crowds.",
 "tok2": "Need to revisit the Tokyo and Kyoto itinerary: the dates moved to April, cherry blossom season.",
 "run": "Started training for a half marathon in February. Three runs a week, one long run on Sunday.",
 "side": "Decision: no more than two side projects at a time. Writing this down so future me can disagree with it properly.",
 "deck": "Presentation software should let slides flow like documents, with layouts chosen by what the content says.",
}
# (earlier, later, same_topic, pushback)
PAIRS = [
 ("pl", "job", 1, 0), ("pl", "own", 1, 0), ("own", "angel", 1, 1), ("own", "own2", 1, 0),
 ("conc", "q", 1, 1), ("mom", "ui", 1, 0), ("sour", "loaf", 1, 0), ("tok", "kyo", 1, 0), ("tok", "tok2", 1, 1),
 ("pl", "sour", 0, 0), ("own", "side", 0, 0), ("ui", "run", 0, 0), ("mom", "own", 0, 0), ("conc", "deck", 0, 0),
 ("kyo", "loaf", 0, 0), ("job2", "angel", 0, 0),
]
Q = {
 "same": {"type": "noul", "instructions": "Are the two notes about the same topic or idea?"},
 "push": {"type": "noul", "instructions": "Does the later note disagree with, question, or revise something in the earlier note?"},
}

def auc(scores, labels):
    pos = [s for s, l in zip(scores, labels) if l]; neg = [s for s, l in zip(scores, labels) if not l]
    return sum((p > n) + 0.5 * (p == n) for p in pos for n in neg) / (len(pos) * len(neg))

router = Router()
for model in ["english", "multilingual"]:
    for style in ["json", "text"]:
        same, push = [], []
        t = time.time()
        for a, b, *_ in PAIRS:
            state = {"earlier_note": N[a], "later_note": N[b]} if style == "json" else f"Earlier note: {N[a]}\n\nLater note: {N[b]}"
            ans = router.predict(state, Q, model=model)["answers"]
            same.append(ans["same"]["noul"]); push.append(ans["push"]["noul"])
        el = (time.time() - t) / len(PAIRS)
        ls = [p[2] for p in PAIRS]; lp = [p[3] for p in PAIRS]
        lp_rel = [(s, l) for s, l, r in zip(push, lp, ls) if r]  # pushback only among same-topic pairs
        print(f"\n## {model} / {style}: {el*1000:.0f} ms per pair (2 questions, CPU)")
        print(f"   same-topic AUC {auc(same, ls):.2f}   pushback AUC (among same-topic) {auc([x[0] for x in lp_rel], [x[1] for x in lp_rel]):.2f}")
        for (a, b, s_, p_), s, p in zip(PAIRS, same, push):
            print(f"   {a:>5}→{b:<5} same {s:.2f} (truth {s_})   push {p:.2f} (truth {p_})")
