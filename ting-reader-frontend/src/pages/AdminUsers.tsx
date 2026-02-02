import React, { useEffect, useState } from 'react';
import apiClient from '../api/client';
import type { User as UserType } from '../types';
import { 
  Plus, 
  User, 
  Users,
  Trash2, 
  Shield, 
  ShieldCheck,
  Calendar,
  X,
  Edit
} from 'lucide-react';

const AdminUsers: React.FC = () => {
  const [users, setUsers] = useState<UserType[]>([]);
  const [loading, setLoading] = useState(true);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  
  const [formData, setFormData] = useState({
    username: '',
    password: '',
    role: 'user' as 'user' | 'admin'
  });

  useEffect(() => {
    fetchUsers();
  }, []);

  const fetchUsers = async () => {
    try {
      const response = await apiClient.get('/api/users');
      setUsers(response.data);
    } catch (err) {
      console.error('Failed to fetch users', err);
    } finally {
      setLoading(false);
    }
  };

  const handleOpenAddModal = () => {
    setEditingId(null);
    setFormData({ username: '', password: '', role: 'user' });
    setIsModalOpen(true);
  };

  const handleOpenEditModal = (user: UserType) => {
    setEditingId(user.id);
    setFormData({ username: user.username, password: '', role: user.role });
    setIsModalOpen(true);
  };

  const handleSaveUser = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      if (editingId) {
        const currentUser = users.find(u => u.id === editingId);
        const updateData: any = {};
        
        if (formData.username !== currentUser?.username) {
          updateData.username = formData.username;
        }
        if (formData.role !== currentUser?.role) {
          updateData.role = formData.role;
        }
        if (formData.password) {
          updateData.password = formData.password;
        }

        if (Object.keys(updateData).length > 0) {
          await apiClient.patch(`/api/users/${editingId}`, updateData);
        }
      } else {
        await apiClient.post('/api/users', formData);
      }
      setIsModalOpen(false);
      setFormData({ username: '', password: '', role: 'user' });
      setEditingId(null);
      fetchUsers();
    } catch (err: any) {
      alert(err.response?.data?.error || '操作失败');
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm('确定要删除此用户吗？')) return;
    try {
      await apiClient.delete(`/api/users/${id}`);
      fetchUsers();
    } catch (err) {
      alert('删除失败');
    }
  };

  return (
    <div className="w-full max-w-screen-2xl mx-auto p-4 sm:p-6 md:p-8 lg:p-10 space-y-8">
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-6">
        <div className="text-center md:text-left">
          <h1 className="text-2xl md:text-3xl font-bold dark:text-white flex items-center justify-center md:justify-start gap-3">
            <Users size={28} className="text-primary-600 md:w-8 md:h-8" />
            用户管理
          </h1>
          <p className="text-sm md:text-base text-slate-500 mt-1">管理系统访问权限与账号</p>
        </div>
        <div className="flex items-center gap-3 w-full md:w-auto">
          <button 
            onClick={() => {
              setEditingId(null);
              setFormData({ username: '', password: '', role: 'user' });
              setIsModalOpen(true);
            }}
            className="flex-1 md:flex-none flex items-center justify-center gap-2 px-4 md:px-6 py-3 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-xl shadow-lg shadow-primary-500/30 transition-all text-sm md:text-base"
          >
            <Plus size={18} className="md:w-5 md:h-5" />
            创建用户
          </button>
        </div>
      </div>

      <div className="bg-white dark:bg-slate-900 rounded-3xl overflow-hidden border border-slate-100 dark:border-slate-800 shadow-sm">
        {/* Desktop Table View */}
        <div className="hidden md:block overflow-x-auto">
          <table className="w-full text-left">
            <thead>
              <tr className="bg-slate-50 dark:bg-slate-800/50 text-slate-500 text-sm font-bold uppercase tracking-wider">
                <th className="px-6 py-4">用户信息</th>
                <th className="px-6 py-4">角色</th>
                <th className="px-6 py-4">创建时间</th>
                <th className="px-6 py-4 text-right">操作</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100 dark:divide-slate-800">
              {users.map((u) => (
                <tr key={u.id} className="hover:bg-slate-50/50 dark:hover:bg-slate-800/30 transition-colors">
                  <td className="px-6 py-4">
                    <div className="flex items-center gap-3">
                      <div className="w-10 h-10 rounded-full bg-primary-100 dark:bg-primary-900/30 flex items-center justify-center text-primary-600 font-bold">
                        {u.username.charAt(0).toUpperCase()}
                      </div>
                      <div>
                        <div className="font-bold dark:text-white">{u.username}</div>
                        <div className="text-xs text-slate-400">ID: {u.id.substring(0, 8)}...</div>
                      </div>
                    </div>
                  </td>
                  <td className="px-6 py-4">
                    <div className={`inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-xs font-bold ${
                      u.role === 'admin' 
                        ? 'bg-purple-100 text-purple-600 dark:bg-purple-900/20 dark:text-purple-400' 
                        : 'bg-blue-100 text-blue-600 dark:bg-blue-900/20 dark:text-blue-400'
                    }`}>
                      {u.role === 'admin' ? <ShieldCheck size={14} /> : <Shield size={14} />}
                      {u.role === 'admin' ? '管理员' : '普通用户'}
                    </div>
                  </td>
                  <td className="px-6 py-4">
                    <div className="flex items-center gap-2 text-sm text-slate-500">
                      <Calendar size={14} />
                      {new Date(u.created_at).toLocaleDateString()}
                    </div>
                  </td>
                  <td className="px-6 py-4 text-right">
                    <div className="flex items-center justify-end gap-2">
                      <button 
                        onClick={() => handleOpenEditModal(u)}
                        className="p-2 text-slate-400 hover:text-primary-600 hover:bg-primary-50 dark:hover:bg-primary-900/20 rounded-lg transition-all"
                      >
                        <Edit size={18} />
                      </button>
                      <button 
                        onClick={() => handleDelete(u.id)}
                        className="p-2 text-slate-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-all"
                      >
                        <Trash2 size={18} />
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        {/* Mobile Card View */}
        <div className="md:hidden divide-y divide-slate-100 dark:divide-slate-800">
          {users.map((u) => (
            <div key={u.id} className="p-4 space-y-4">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <div className="w-12 h-12 rounded-2xl bg-primary-100 dark:bg-primary-900/30 flex items-center justify-center text-primary-600 font-bold text-lg">
                    {u.username.charAt(0).toUpperCase()}
                  </div>
                  <div>
                    <div className="font-bold text-slate-900 dark:text-white">{u.username}</div>
                    <div className="text-[10px] text-slate-400 uppercase tracking-tight">ID: {u.id.substring(0, 8)}</div>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <button 
                    onClick={() => handleOpenEditModal(u)}
                    className="p-2.5 text-slate-400 hover:text-primary-600 bg-slate-50 dark:bg-slate-800 rounded-xl transition-all"
                  >
                    <Edit size={20} />
                  </button>
                  <button 
                    onClick={() => handleDelete(u.id)}
                    className="p-2.5 text-slate-400 hover:text-red-500 bg-slate-50 dark:bg-slate-800 rounded-xl transition-all"
                  >
                    <Trash2 size={20} />
                  </button>
                </div>
              </div>
              
              <div className="flex items-center justify-between pt-2">
                <div className={`inline-flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-xs font-bold ${
                  u.role === 'admin' 
                    ? 'bg-purple-100 text-purple-600 dark:bg-purple-900/20 dark:text-purple-400' 
                    : 'bg-blue-100 text-blue-600 dark:bg-blue-900/20 dark:text-blue-400'
                }`}>
                  {u.role === 'admin' ? <ShieldCheck size={14} /> : <Shield size={14} />}
                  {u.role === 'admin' ? '管理员' : '普通用户'}
                </div>
                <div className="flex items-center gap-1.5 text-xs text-slate-400 font-medium">
                  <Calendar size={14} />
                  {new Date(u.created_at).toLocaleDateString()}
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Add User Modal */}
      {isModalOpen && (
        <div className="fixed inset-0 z-[200] flex items-center justify-center p-4">
          <div className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm" onClick={() => setIsModalOpen(false)}></div>
          <div className="relative w-full max-w-md bg-white dark:bg-slate-900 rounded-3xl shadow-2xl overflow-hidden animate-in zoom-in-95 duration-200">
            <div className="p-8">
              <div className="flex items-center justify-between mb-6">
                <h2 className="text-2xl font-bold dark:text-white">{editingId ? '修改用户信息' : '创建新账号'}</h2>
                <button onClick={() => setIsModalOpen(false)} className="text-slate-400 hover:text-slate-600">
                  <X size={24} />
                </button>
              </div>
              <form onSubmit={handleSaveUser} className="space-y-5">
                <div className="space-y-2">
                  <label className="text-sm font-bold text-slate-600 dark:text-slate-400">用户名</label>
                  <input 
                    type="text" 
                    required
                    value={formData.username}
                    onChange={e => setFormData({...formData, username: e.target.value})}
                    className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-bold text-slate-600 dark:text-slate-400">{editingId ? '新密码 (留空则不修改)' : '初始密码'}</label>
                  <input 
                    type="password" 
                    required={!editingId}
                    value={formData.password}
                    onChange={e => setFormData({...formData, password: e.target.value})}
                    className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-bold text-slate-600 dark:text-slate-400">权限角色</label>
                  <div className="grid grid-cols-2 gap-3">
                    <button
                      type="button"
                      onClick={() => setFormData({...formData, role: 'user'})}
                      className={`py-3 rounded-xl font-bold transition-all border ${
                        formData.role === 'user' 
                          ? 'bg-primary-50 border-primary-200 text-primary-600' 
                          : 'bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 text-slate-400'
                      }`}
                    >
                      普通用户
                    </button>
                    <button
                      type="button"
                      onClick={() => setFormData({...formData, role: 'admin'})}
                      className={`py-3 rounded-xl font-bold transition-all border ${
                        formData.role === 'admin' 
                          ? 'bg-purple-50 border-purple-200 text-purple-600' 
                          : 'bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 text-slate-400'
                      }`}
                    >
                      管理员
                    </button>
                  </div>
                </div>

                <button 
                  type="submit"
                  className="w-full py-4 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-xl shadow-lg shadow-primary-500/30 transition-all mt-4"
                >
                  {editingId ? '保存修改' : '立即创建'}
                </button>
              </form>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default AdminUsers;
