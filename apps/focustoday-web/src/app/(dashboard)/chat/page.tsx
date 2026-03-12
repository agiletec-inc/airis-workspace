import { ChatPanel } from '@/components/chat/chat-panel'

export default function ChatPage() {
  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center border-b px-6 py-4">
        <h1 className="text-xl font-semibold">AI Chat</h1>
      </header>
      <div className="flex-1 overflow-hidden">
        <ChatPanel />
      </div>
    </div>
  )
}
