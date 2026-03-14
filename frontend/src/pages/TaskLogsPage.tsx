import React, { useEffect, useState } from 'react';
import apiClient from '../api/client';
import { 
  Terminal, 
  RefreshCw, 
  CheckCircle2, 
  XCircle, 
  Clock, 
  Loader2,
  Database,
  Search,
  Trash2,
  StopCircle,
  CheckSquare,
  FileSignature
} from 'lucide-react';
import { formatDate } from '../utils/date';
import { formatTaskPayload, getTaskStatusText } from '../utils/task';

interface Task {
  id: string;
  taskType: string;
  status: 'queued' | 'running' | 'completed' | 'failed' | 'cancelled';
  payload: string;
  message?: string;
  error?: string;
  createdAt: string;
  updatedAt: string;
}

const TaskLogsPage: React.FC = () => {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [loading, setLoading] = useState(true);
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [selectedTaskIds, setSelectedTaskIds] = useState<Set<string>>(new Set());

  const [isSelectionMode, setIsSelectionMode] = useState(false);

  useEffect(() => {
    const fetchTasks = async () => {
      try {
        const response = await apiClient.get('/api/tasks');
        setTasks(response.data);
        // 清理已不存在的任务ID选中状态
        setSelectedTaskIds(prev => {
          const newSet = new Set();
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          response.data.forEach((t: any) => {
              if (prev.has(t.id)) newSet.add(t.id);
          });
          // 如果没有选中项，退出选择模式
          if (newSet.size === 0 && isSelectionMode) {
              // 这里不自动退出，因为可能是刷新导致列表暂时为空，或者用户刚刚清空了选择
              // 但如果列表本身为空，可以退出
          }
          return newSet as Set<string>;
        });
      } catch (err) {
        console.error('Failed to fetch tasks', err);
      } finally {
        setLoading(false);
      }
    };

    fetchTasks();
    let interval: ReturnType<typeof setInterval>;
    if (autoRefresh) {
      interval = setInterval(fetchTasks, 3000);
    }
    return () => clearInterval(interval);
  }, [autoRefresh, isSelectionMode]);

  const manualFetchTasks = async () => {
    try {
      const response = await apiClient.get('/api/tasks');
      setTasks(response.data);
      // 清理已不存在的任务ID选中状态
      setSelectedTaskIds(prev => {
        const newSet = new Set();
        response.data.forEach((t: Task) => {
            if (prev.has(t.id)) newSet.add(t.id);
        });
        // 如果没有选中项，退出选择模式
        if (newSet.size === 0 && isSelectionMode) {
            // 这里不自动退出，因为可能是刷新导致列表暂时为空，或者用户刚刚清空了选择
            // 但如果列表本身为空，可以退出
        }
        return newSet as Set<string>;
      });
    } catch (err) {
      console.error('Failed to fetch tasks', err);
    } finally {
      setLoading(false);
    }
  };

  const handleCancel = async (taskId: string) => {
    try {
      await apiClient.post(`/api/tasks/${taskId}/cancel`);
      manualFetchTasks();
    } catch (err) {
      console.error('Failed to cancel task', err);
    }
  };

  const handleDelete = async (taskId: string) => {
    if (!confirm('确定要删除这条任务记录吗？')) return;
    try {
      await apiClient.delete(`/api/tasks/${taskId}`);
      manualFetchTasks();
    } catch (err) {
      console.error('Failed to delete task', err);
    }
  };

  const handleBatchDelete = async () => {
    if (selectedTaskIds.size === 0) return;
    if (!confirm(`确定要删除选中的 ${selectedTaskIds.size} 条任务记录吗？`)) return;
    
    try {
        await apiClient.post('/api/tasks/batch-delete', { ids: Array.from(selectedTaskIds) });
        setSelectedTaskIds(new Set());
        setIsSelectionMode(false);
        manualFetchTasks();
    } catch (err) {
        console.error('Failed to batch delete tasks', err);
    }
  };

  const toggleSelect = (id: string) => {
    const newSet = new Set(selectedTaskIds);
    if (newSet.has(id)) {
        newSet.delete(id);
    } else {
        newSet.add(id);
    }
    setSelectedTaskIds(newSet);
    
    // 如果没有选中项，退出选择模式
    if (newSet.size === 0) {
        setIsSelectionMode(false);
    }
  };

  const toggleSelectAll = () => {
    const deletableTasks = tasks.filter(t => t.status !== 'running');
    if (selectedTaskIds.size === deletableTasks.length && deletableTasks.length > 0) {
        setSelectedTaskIds(new Set());
        setIsSelectionMode(false);
    } else {
        const newSet = new Set(deletableTasks.map(t => t.id));
        setSelectedTaskIds(newSet);
    }
  };

  const toggleSelectionMode = () => {
    if (isSelectionMode) {
        setSelectedTaskIds(new Set());
        setIsSelectionMode(false);
    } else {
        setIsSelectionMode(true);
    }
  };

  const handleClearAll = async () => {
    if (!confirm('确定要清空所有非运行中的任务记录吗？')) return;
    try {
      await apiClient.delete('/api/tasks');
      manualFetchTasks();
    } catch (err) {
      console.error('Failed to clear tasks', err);
    }
  };

  const getStatusIcon = (status: Task['status']) => {
    switch (status) {
      case 'completed': return <CheckCircle2 className="text-green-500" size={20} />;
      case 'failed': return <XCircle className="text-red-500" size={20} />;
      case 'running': return <Loader2 className="text-blue-500 animate-spin" size={20} />;
      case 'cancelled': return <XCircle className="text-gray-400" size={20} />;
      default: return <Clock className="text-slate-400" size={20} />;
    }
  };

  if (loading && tasks.length === 0) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary-600"></div>
      </div>
    );
  }

  const deletableTasks = tasks.filter(t => t.status !== 'running');
  const isAllSelected = deletableTasks.length > 0 && selectedTaskIds.size === deletableTasks.length;
  const isIndeterminate = selectedTaskIds.size > 0 && selectedTaskIds.size < deletableTasks.length;

  return (
    <div className="w-full max-w-screen-2xl mx-auto p-4 sm:p-6 md:p-8 lg:p-10 space-y-8">
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-6">
        <div className="text-center md:text-left">
          <h1 className="text-2xl md:text-3xl font-bold dark:text-white flex items-center justify-center md:justify-start gap-3">
            <Terminal size={28} className="text-primary-600 md:w-8 md:h-8" />
            任务日志
          </h1>
          <p className="text-sm md:text-base text-slate-500 mt-1">实时监控系统扫描与刮削进度</p>
        </div>
        <div className="flex items-center justify-center gap-2 sm:gap-4 bg-white dark:bg-slate-900 p-2 sm:p-3 rounded-2xl border border-slate-100 dark:border-slate-800 shadow-sm md:shadow-none md:border-none md:p-0 md:bg-transparent flex-wrap sm:flex-nowrap">
          {/* Batch Actions */}
          {isSelectionMode && selectedTaskIds.size > 0 && (
            <button
              onClick={handleBatchDelete}
              className="flex items-center gap-1.5 sm:gap-2 px-2 py-1.5 sm:px-3 bg-red-50 text-red-600 rounded-lg hover:bg-red-100 transition-colors text-xs sm:text-sm font-medium mr-1 sm:mr-2"
            >
              <Trash2 size={14} className="sm:w-4 sm:h-4" />
              <span>删除<span className="hidden sm:inline">选中</span> ({selectedTaskIds.size})</span>
            </button>
          )}

          <div className="flex items-center gap-1.5 sm:gap-2 text-xs sm:text-sm text-slate-500">
            <span className="font-bold md:font-normal whitespace-nowrap"><span className="hidden sm:inline">自动</span>刷新</span>
            <button
              onClick={() => setAutoRefresh(!autoRefresh)}
              className={`w-9 h-5 sm:w-12 sm:h-6 rounded-full transition-all relative ${
                autoRefresh ? 'bg-primary-600' : 'bg-slate-200 dark:bg-slate-700'
              }`}
            >
              <div className={`absolute top-0.5 sm:top-1 w-4 h-4 bg-white rounded-full transition-all shadow-sm ${
                autoRefresh ? 'left-[18px] sm:left-7' : 'left-0.5 sm:left-1'
              }`} />
            </button>
          </div>
          
          <button
            onClick={toggleSelectionMode}
            className={`p-2 sm:p-2.5 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl transition-colors shadow-sm ${
                isSelectionMode ? 'text-primary-600 bg-primary-50 border-primary-200' : 'text-slate-600 dark:text-slate-400 hover:bg-slate-50'
            }`}
            title={isSelectionMode ? "退出选择" : "选择任务"}
          >
            <CheckSquare size={18} className="sm:w-5 sm:h-5" />
          </button>

          <button
            onClick={handleClearAll}
            className="p-2 sm:p-2.5 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl text-red-500 hover:bg-red-50 hover:border-red-200 transition-colors shadow-sm"
            title="清空日志"
          >
            <Trash2 size={18} className="sm:w-5 sm:h-5" />
          </button>
          <button 
            onClick={manualFetchTasks}
            className="p-2 sm:p-2.5 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl text-slate-600 dark:text-slate-400 hover:bg-slate-50 transition-colors shadow-sm"
          >
            <RefreshCw size={18} className={`sm:w-5 sm:h-5 ${loading ? 'animate-spin' : ''}`} />
          </button>
        </div>
      </div>

      <div className="bg-white dark:bg-slate-900 rounded-3xl overflow-hidden border border-slate-100 dark:border-slate-800 shadow-sm">
        {/* Header with Select All */}
        {tasks.length > 0 && isSelectionMode && (
          <div className="px-6 py-3 border-b border-slate-100 dark:border-slate-800 flex items-center bg-slate-50/50 dark:bg-slate-800/30">
            <div className="flex items-center gap-4">
              <div className="relative flex items-center">
                <input
                  type="checkbox"
                  className="w-5 h-5 rounded border-slate-300 text-primary-600 focus:ring-primary-500 cursor-pointer"
                  checked={isAllSelected}
                  ref={input => {
                    if (input) input.indeterminate = isIndeterminate;
                  }}
                  onChange={toggleSelectAll}
                  disabled={deletableTasks.length === 0}
                />
              </div>
              <span className="text-sm font-medium text-slate-500">
                {selectedTaskIds.size > 0 ? `已选择 ${selectedTaskIds.size} 项` : '全选未运行任务'}
              </span>
            </div>
          </div>
        )}

        <div className="divide-y divide-slate-100 dark:divide-slate-800">
          {tasks.map((task) => (
            <div key={task.id} className={`p-4 sm:p-6 transition-colors ${
              selectedTaskIds.has(task.id) ? 'bg-blue-50/50 dark:bg-blue-900/10' : 'hover:bg-slate-50/50 dark:hover:bg-slate-800/30'
            }`}>
              <div className="flex flex-col sm:flex-row items-start justify-between gap-4">
                <div className="flex items-start gap-4 w-full sm:w-auto">
                  {isSelectionMode && (
                    <div className="flex items-center h-10 sm:h-12 shrink-0">
                      <input
                        type="checkbox"
                        className="w-5 h-5 sm:w-5 sm:h-5 rounded border-slate-300 text-primary-600 focus:ring-primary-500 cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed scale-90 sm:scale-100"
                        checked={selectedTaskIds.has(task.id)}
                        onChange={() => toggleSelect(task.id)}
                        disabled={task.status === 'running'}
                      />
                    </div>
                  )}
                  
                  <div className={`w-10 h-10 sm:w-12 sm:h-12 rounded-xl flex items-center justify-center shrink-0 ${
                    task.taskType === 'scan' ? 'bg-blue-50 text-blue-600 dark:bg-blue-900/20' : 
                    task.taskType === 'write_metadata' ? 'bg-amber-50 text-amber-600 dark:bg-amber-900/20' :
                    'bg-purple-50 text-purple-600 dark:bg-purple-900/20'
                  }`}>
                    {task.taskType === 'scan' ? <Database size={20} className="sm:w-6 sm:h-6" /> : 
                     task.taskType === 'write_metadata' ? <FileSignature size={20} className="sm:w-6 sm:h-6" /> :
                     <Search size={20} className="sm:w-6 sm:h-6" />}
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2 sm:gap-3 mb-1">
                      <span className="font-bold text-sm sm:text-base dark:text-white truncate">
                        {task.taskType === 'scan' ? '库扫描任务' : 
                         task.taskType === 'write_metadata' ? '元数据写入' : '刮削任务'}
                      </span>
                      <span className={`text-[10px] font-bold uppercase tracking-widest px-2 py-0.5 rounded-md shrink-0 ${
                        task.status === 'completed' ? 'bg-green-100 text-green-600 dark:bg-green-900/20 dark:text-green-400' :
                        task.status === 'failed' ? 'bg-red-100 text-red-600 dark:bg-red-900/20 dark:text-red-400' :
                        'bg-blue-100 text-blue-600 dark:bg-blue-900/20 dark:text-blue-400'
                      }`}>
                        {getTaskStatusText(task.status)}
                      </span>
                    </div>
                    <p className="text-xs sm:text-sm text-slate-500 break-all">{formatTaskPayload(task.payload)}</p>
                    {task.message && (
                      <p className="text-xs sm:text-sm font-medium text-primary-600 dark:text-primary-400 mt-2 flex items-center gap-2">
                      <Loader2 size={12} className={`sm:w-3.5 sm:h-3.5 ${task.status === 'running' ? 'animate-spin' : ''}`} />
                      <span className="truncate whitespace-normal sm:whitespace-nowrap">{task.message}</span>
                    </p>
                    )}
                    {task.error && (
                      <p className="text-xs text-red-500 mt-2 bg-red-50 dark:bg-red-900/10 p-2 rounded-lg border border-red-100 dark:border-red-900/20 break-all">
                        错误: {task.error}
                      </p>
                    )}
                  </div>
                </div>
                
                <div className="flex sm:flex-col items-center sm:items-end justify-between w-full sm:w-auto mt-2 sm:mt-0 pt-2 sm:pt-0 border-t border-slate-100 dark:border-slate-800 sm:border-none">
                  <div className="flex items-center gap-2 sm:mb-1 order-2 sm:order-1">
                    <span className="text-xs text-slate-500 sm:hidden">{getTaskStatusText(task.status)}</span>
                    {getStatusIcon(task.status)}
                  </div>
                  
                  <div className="flex items-center gap-2 order-3 sm:order-2 mt-1 mb-1">
                    {(task.status === 'running' || task.status === 'queued') ? (
                      <button
                        onClick={() => handleCancel(task.id)}
                        className="p-1.5 text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-colors"
                        title="停止任务"
                      >
                        <StopCircle size={18} />
                      </button>
                    ) : (
                      <button
                        onClick={() => handleDelete(task.id)}
                        className="p-1.5 text-slate-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-colors"
                        title="删除记录"
                      >
                        <Trash2 size={18} />
                      </button>
                    )}
                  </div>

                  <div className="text-xs text-slate-400 order-1 sm:order-3">
                    {formatDate(task.createdAt)}
                  </div>
                </div>
              </div>
            </div>
          ))}
        </div>
        
        {tasks.length === 0 && (
          <div className="py-20 text-center">
            <Terminal size={48} className="mx-auto text-slate-200 mb-4" />
            <p className="text-slate-500 font-medium">暂无任务日志</p>
          </div>
        )}
      </div>
    </div>
  );
};

export default TaskLogsPage;
