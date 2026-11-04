---
layout: ../../layouts/Guide.astro
title: Chat and models
description: How to chat with your notes in Noughtpad, choose a model that runs on your computer or your own provider (Anthropic, OpenAI, Gemini, Ollama), and control which notes are sent.
---

Chat lets you ask questions about your notes in plain language: "How has my view on this changed?", "What did I leave open in October?", "What have I read about sleep?". Answers point to the notes they used.

Chat is **off** until you choose a model. Nothing is sent anywhere until then.

## Choose a model

Open **Models**: press **⌘K** and type "models", or go to **Settings → Chat → Language models**. There are two kinds.

### A model on your computer

Free, private, and works without internet. Your notes never leave your computer.

| Model | Download | Good for |
|---|---|---|
| Qwen3 4B Instruct (recommended) | 2.5 GB | Most laptops. Cites your notes reliably. |
| Qwen3 1.7B | 1.1 GB | Older or smaller computers. Faster, less careful. |
| Qwen3.5 4B | 2.7 GB | Computers with a strong graphics chip. |

1. Click **Download** next to a model. You can pause and resume; if your connection drops, it carries on from where it stopped.
2. When it's done, click **Use**.

Downloading doesn't switch anything on: only **Use** does. The first answer after starting Noughtpad takes longer, often 20 seconds or so, while the model loads. Later answers are quicker.

**If the model won't load:** another app may be using your computer's graphics memory. Ollama and LM Studio often keep a model loaded. Quit them, or unload their model, and try again.

### Your own provider

If you have an account with an AI provider, you can use it instead:

- **Anthropic**, **OpenAI** or **Gemini**: paste your API key.
- **Ollama**: no key needed, just the server address.
- **Any OpenAI-compatible server**, such as LM Studio or OpenRouter: the server address, and a key if it needs one.

Click **Add**, pick a model from the list, then click **Use**. Keys are kept in your computer's own keychain, never in your notes folder. On a Linux computer without a keychain, the key is kept only until you quit, and the form tells you so.

The provider charges you for what you use, as it normally would. Noughtpad adds nothing.

## Start a chat

- Click **Chat** in the sidebar, or press **⌘L** (Ctrl+L).
- Or click **Ask about this note…** in the related-notes panel.
- Or click **Ask how your thinking on this changed…** on a thread in Ideas over time.

## What the chat can read

Above the question box, pick what the chat may look at:

- **Whole notebook**
- **This note** and its related notes
- A **thread** from Ideas over time
- **This month**
- **Open questions**
- A **collection** (one label)

With an online provider, the model searches and reads notes itself, but only within that choice. Each search and each note it reads appears as a step you can expand. With a model on your computer, Noughtpad picks the six most relevant notes for each question.

Notes are numbered the first time they come up, and answers cite them as **[1]**, **[2]** and so on. Click a number to open the note.

## Before anything leaves your computer

The first time a chat would send notes to an online provider, Noughtpad stops and shows:

- where they would go (the provider and model), and
- which notes are involved.

You can untick any note. Nothing is sent until you approve. It asks again for each new chat, and whenever the destination changes. Models on your computer never ask, because nothing leaves it.

## Your chats

Past chats are listed in the panel on the right of the chat page (the clock button at the top right shows or hides it). Click one to reopen it and carry on. Chats are saved in your notebook folder, in `.noughtpad/chats`. A chat never changes your notes.
