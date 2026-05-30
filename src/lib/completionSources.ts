import { type CompletionContext, type CompletionResult, type Completion } from '@codemirror/autocomplete'
import { ensureSyntaxTree } from '@codemirror/language'
import { useAppStore } from '../hooks/useAppStore'

const COMMANDS = [
  'search', 'summarize', 'fetch-papers', 'deep-dive', 'explain', 'diff-review', 'status', 'config', 'dream',
  'rewrite', 'translate', 'expand', 'simplify', 'brainstorm', 'outline',
  'organize', 'connect', 'compare', 'question', 'suggest', 'review',
  'format', 'extract', 'visualize'
]

export function getAllCommandCompletions(): Completion[] {
  return COMMANDS.map((cmd) => ({
    label: '/' + cmd,
    type: 'keyword',
    detail: 'command',
    apply: '/' + cmd + ' '
  }))
}

export function slashCommandSource(context: CompletionContext): CompletionResult | null {
  const word = context.matchBefore(/\/[a-zA-Z-]*/)
  if (!word) return null
  if (word.from === word.to && !context.explicit) return null

  const filter = word.text.slice(1).toLowerCase()
  const options = COMMANDS
    .filter((cmd) => !filter || cmd.startsWith(filter))
    .map((cmd) => ({
      label: '/' + cmd,
      type: 'keyword' as const,
      detail: 'command',
      apply: '/' + cmd + ' '
    }))

  if (options.length === 0) return null

  return {
    from: word.from,
    options,
    validFor: /\/[\w-]*/
  }
}

export function wikiLinkSource(context: CompletionContext): CompletionResult | null {
  const word = context.matchBefore(/\[\[[^[\]\n]*$/)
  if (!word) return null

  const filter = word.text.slice(2).toLowerCase()
  const notes = useAppStore.getState().notes

  const options = notes
    .filter((n) => !filter || n.title.toLowerCase().includes(filter))
    .map((n) => ({
      label: n.title,
      type: 'class' as const,
      detail: n.path,
      apply: n.title + ']]'
    }))

  if (options.length === 0) return null

  return {
    from: word.from + 2,
    options,
    validFor: /^[^[\]\n]*$/
  }
}

export function headingSource(context: CompletionContext): CompletionResult | null {
  const word = context.matchBefore(/^#{1,6}\s/)
  if (!word) return null

  const tree = ensureSyntaxTree(context.state, context.state.doc.length, 500)
  if (!tree) return null

  const headings: string[] = []
  tree.iterate({
    enter(node) {
      const name = node.type.name
      if (/^(ATX|Setext)Heading\d$/.test(name)) {
        const text = context.state.doc.sliceString(node.from, node.to)
        headings.push(text.replace(/^#{1,6}\s*/, ''))
      }
    }
  })

  const filter = word.text.replace(/^#{1,6}\s*/, '').toLowerCase()
  const seen = new Set<string>()
  const options = headings
    .filter((h) => {
      const key = h.toLowerCase()
      if (seen.has(key)) return false
      seen.add(key)
      return !filter || h.toLowerCase().includes(filter)
    })
    .map((h) => ({
      label: h,
      type: 'text' as const,
      detail: 'heading'
    }))

  if (options.length === 0) return null

  const markerLen = word.text.match(/^#{1,6}\s+/)?.[0].length || 0
  return {
    from: word.from + markerLen,
    options,
    validFor: /.*/
  }
}

export function tagSource(context: CompletionContext): CompletionResult | null {
  const word = context.matchBefore(/#[\w\u4e00-\u9fff/-]*$/)
  if (!word) return null
  if (word.from === word.to && !context.explicit) return null

  const filter = word.text.slice(1).toLowerCase()
  const allTags = useAppStore.getState().allTags || []

  const options = allTags
    .filter((t) => !filter || t.name.toLowerCase().includes(filter))
    .sort((a, b) => b.count - a.count)
    .slice(0, 20)
    .map((t) => ({
      label: t.name,
      type: 'text' as const,
      detail: `${t.count} notes`,
      apply: t.name + ' '
    }))

  if (options.length === 0) return null

  return {
    from: word.from + 1,
    options,
    validFor: /^[\w\u4e00-\u9fff/-]*$/
  }
}
