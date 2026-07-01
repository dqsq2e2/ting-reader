import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import apiClient from '../../core/api/client';
import type { User as UserType, Library, Book, Series } from '../../core/types';
import { 
  Plus, 
  Users,
  Trash2, 
  Shield, 
  ShieldCheck,
  Calendar,
  X,
  Edit
} from 'lucide-react';
import { formatDate } from '../../core/utils/date';

const AdminUsers: React.FC = () => {
  const { t } = useTranslation();
  const [users, setUsers] = useState<UserType[]>([]);
  // const [loading, setLoading] = useState(true);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  
  const [libraries, setLibraries] = useState<Library[]>([]);
  const [formData, setFormData] = useState({
    username: '',
    password: '',
    role: 'user' as 'user' | 'admin',
    libraries_accessible: [] as string[],
    books_accessible: [] as string[]
  });
  
  // Book/Series Search
  const [bookSearchQuery, setBookSearchQuery] = useState('');
  const [bookSearchResults, setBookSearchResults] = useState<Book[]>([]);
  const [seriesSearchResults, setSeriesSearchResults] = useState<Series[]>([]);
  const [selectedBooks, setSelectedBooks] = useState<Book[]>([]);
  const [isSearchingBooks, setIsSearchingBooks] = useState(false);

  useEffect(() => {
    fetchUsers();
    fetchLibraries();
  }, []);

  const fetchLibraries = async () => {
    try {
      const response = await apiClient.get('/api/libraries');
      setLibraries(response.data);
    } catch (err) {
      console.error('Failed to load libraries', err);
    }
  };

  const fetchUsers = async () => {
    try {
      const response = await apiClient.get('/api/users');
      setUsers(response.data);
    } catch (err) {
      console.error('Failed to load users', err);
    } finally {
      // setLoading(false);
    }
  };

  useEffect(() => {
    if (bookSearchQuery.trim().length > 0) {
      const timer = setTimeout(async () => {
        setIsSearchingBooks(true);
        try {
          const [booksRes, seriesRes] = await Promise.all([
            apiClient.get('/api/books', { params: { search: bookSearchQuery } }),
            apiClient.get('/api/v1/series')
          ]);
          
          setBookSearchResults(booksRes.data.slice(0, 10)); // Limit to 10
          
          const filteredSeries = (seriesRes.data as Series[]).filter(s => 
            s.title.toLowerCase().includes(bookSearchQuery.toLowerCase()) || 
            (s.author && s.author.toLowerCase().includes(bookSearchQuery.toLowerCase()))
          );
          setSeriesSearchResults(filteredSeries.slice(0, 5)); // Limit to 5
          
        } catch (err) {
          console.error('Failed to search books or series', err);
        } finally {
          setIsSearchingBooks(false);
        }
      }, 500);
      return () => clearTimeout(timer);
    } else {
      setBookSearchResults([]);
      setSeriesSearchResults([]);
    }
  }, [bookSearchQuery]);

  /*
  const handleOpenAddModal = () => {
    setEditingId(null);
    setFormData({ 
      username: '', 
      password: '', 
      role: 'user', 
      librariesAccessible: [], 
      booksAccessible: [] 
    });
    setSelectedBooks([]);
    setBookSearchQuery('');
    setIsModalOpen(true);
  };
  */

  const handleOpenEditModal = async (user: UserType) => {
    setEditingId(user.id);
    const booksAccessible = Array.isArray(user.books_accessible) ? user.books_accessible : [];
    
    // Fetch details for selected books to display names
    const books = [];
    if (booksAccessible.length > 0) {
      // This is suboptimal (N requests), but simple. 
      // Better would be a bulk fetch endpoint or relying on client cache if available.
      // For now, let's just fetch them one by one or maybe we don't need details if we only show IDs?
      // No, we need names. Let's try to fetch all books and filter? No, too heavy.
      // Let's just fire requests.
      for (const bid of booksAccessible) {
        try {
          const res = await apiClient.get(`/api/books/${bid}`);
          books.push(res.data);
        } catch {
            // ignore
        }
      }
    }
    setSelectedBooks(books);

    setFormData({ 
      username: user.username, 
      password: '', 
      role: user.role,
      libraries_accessible: Array.isArray(user.libraries_accessible) ? user.libraries_accessible : [],
      books_accessible: booksAccessible
    });
    setBookSearchQuery('');
    setIsModalOpen(true);
  };

  const handleSaveUser = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      const payload = { ...formData };
      
      // If admin, they have access to all, so we don't need to send specific libraries
      if (payload.role === 'admin') {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        delete (payload as any).libraries_accessible;
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        delete (payload as any).books_accessible;
      }

      if (editingId) {
        const currentUser = users.find(u => u.id === editingId);
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const updateData: Record<string, any> = {};
        
        if (payload.username !== currentUser?.username) {
          updateData.username = payload.username;
        }
        if (payload.role !== currentUser?.role) {
          updateData.role = payload.role;
        }
        if (payload.password) {
          updateData.password = payload.password;
        }
        // Always send permissions if role is user, to ensure sync
        if (payload.role === 'user') {
          updateData.libraries_accessible = payload.libraries_accessible;
          updateData.books_accessible = payload.books_accessible;
        }

        if (Object.keys(updateData).length > 0) {
          await apiClient.patch(`/api/users/${editingId}`, updateData);
        }
      } else {
        await apiClient.post('/api/users', payload);
      }
      setIsModalOpen(false);
      setFormData({
        username: '',
        password: '',
        role: 'user',
        libraries_accessible: [],
        books_accessible: []
      });
      setEditingId(null);
      fetchUsers();
    } catch (err: unknown) {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const msg = (err as any)?.response?.data?.error || t('adminUsers.operationFailed');
        alert(msg);
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm(t('adminUsers.deleteConfirm'))) return;
    try {
      await apiClient.delete(`/api/users/${id}`);
      fetchUsers();
    } catch {
      alert(t('adminUsers.deleteFailed'));
    }
  };

  return (
    <div className="w-full max-w-screen-2xl mx-auto p-4 sm:p-6 md:p-8 lg:p-10 space-y-8">
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-6">
        <div className="text-center md:text-left">
          <h1 className="text-2xl md:text-3xl font-bold dark:text-white flex items-center justify-center md:justify-start gap-3">
            <Users size={28} className="text-primary-600 md:w-8 md:h-8" />
            {t('adminUsers.title')}
          </h1>
          <p className="text-sm md:text-base text-slate-500 mt-1">{t('adminUsers.subtitle')}</p>
        </div>
        <div className="flex items-center gap-3 w-full md:w-auto">
          <button 
            onClick={() => {
              setEditingId(null);
              setFormData({ 
                username: '', 
                password: '', 
                role: 'user',
                libraries_accessible: [],
                books_accessible: []
              });
              setIsModalOpen(true);
            }}
            className="flex-1 md:flex-none flex items-center justify-center gap-2 px-4 md:px-6 py-3 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-xl shadow-lg shadow-primary-500/30 transition-all text-sm md:text-base"
          >
            <Plus size={18} className="md:w-5 md:h-5" />
            {t('adminUsers.createUser')}
          </button>
        </div>
      </div>

      <div className="bg-white dark:bg-slate-900 rounded-3xl overflow-hidden border border-slate-100 dark:border-slate-800 shadow-sm">
        {/* Desktop Table View */}
        <div className="hidden md:block overflow-x-auto">
          <table className="w-full text-left">
            <thead>
              <tr className="bg-slate-50 dark:bg-slate-800/50 text-slate-500 text-sm font-bold uppercase tracking-wider">
                <th className="px-6 py-4">{t('adminUsers.userInfo')}</th>
                <th className="px-6 py-4">{t('adminUsers.role')}</th>
                <th className="px-6 py-4">{t('adminUsers.createdAt')}</th>
                <th className="px-6 py-4 text-right">{t('adminUsers.actions')}</th>
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
                      {u.role === 'admin' ? t('adminUsers.admin') : t('adminUsers.regularUser')}
                    </div>
                  </td>
                  <td className="px-6 py-4">
                    <div className="flex items-center gap-2 text-sm text-slate-500">
                      <Calendar size={14} />
                      {formatDate(u.created_at)}
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
                  {u.role === 'admin' ? t('adminUsers.admin') : t('adminUsers.regularUser')}
                </div>
                <div className="flex items-center gap-1.5 text-xs text-slate-400 font-medium">
                  <Calendar size={14} />
                  {formatDate(u.created_at)}
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
          <div className="relative w-full max-w-md bg-white dark:bg-slate-900 rounded-3xl shadow-2xl overflow-hidden animate-in zoom-in-95 duration-200 flex flex-col max-h-[85vh]">
            <div className="p-8 overflow-y-auto flex-1">
              <div className="flex items-center justify-between mb-6">
                <h2 className="text-2xl font-bold dark:text-white">{editingId ? t('adminUsers.editUser') : t('adminUsers.createAccount')}</h2>
                <button onClick={() => setIsModalOpen(false)} className="text-slate-400 hover:text-slate-600">
                  <X size={24} />
                </button>
              </div>
              <form onSubmit={handleSaveUser} className="space-y-5">
                <div className="space-y-2">
                  <label className="text-sm font-bold text-slate-600 dark:text-slate-400">{t('adminUsers.username')}</label>
                  <input 
                    type="text" 
                    required
                    value={formData.username}
                    onChange={e => setFormData({...formData, username: e.target.value})}
                    className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-bold text-slate-600 dark:text-slate-400">{editingId ? t('adminUsers.newPasswordHint') : t('adminUsers.initialPassword')}</label>
                  <input 
                    type="password" 
                    required={!editingId}
                    value={formData.password}
                    onChange={e => setFormData({...formData, password: e.target.value})}
                    className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-bold text-slate-600 dark:text-slate-400">{t('adminUsers.permissionRole')}</label>
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
                      {t('adminUsers.regularUser')}
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
                      {t('adminUsers.admin')}
                    </button>
                  </div>
                </div>

                {formData.role === 'user' && (
                  <>
                    <div className="space-y-2">
                      <label className="text-sm font-bold text-slate-600 dark:text-slate-400">{t('adminUsers.accessibleLibraries')}</label>
                      <div className="space-y-2 max-h-40 overflow-y-auto p-3 bg-slate-50 dark:bg-slate-800 rounded-xl border border-slate-200 dark:border-slate-700">
                        {libraries.length > 0 ? libraries.map(lib => (
                          <label key={lib.id} className="flex items-center gap-2 cursor-pointer">
                            <input
                              type="checkbox"
                              checked={(formData.libraries_accessible || []).includes(lib.id)}
                              onChange={(e) => {
                                const checked = e.target.checked;
                                setFormData(prev => {
                                  const current = prev.libraries_accessible || [];
                                  if (checked) {
                                    return { ...prev, libraries_accessible: [...current, lib.id] };
                                  } else {
                                    return { ...prev, libraries_accessible: current.filter((id: string) => id !== lib.id) };
                                  }
                                });
                              }}
                              className="rounded border-slate-300 text-primary-600 focus:ring-primary-500"
                            />
                            <span className="text-sm text-slate-700 dark:text-slate-300">{lib.name}</span>
                          </label>
                        )) : (
                          <p className="text-xs text-slate-400">{t('adminUsers.noLibraries')}</p>
                        )}
                      </div>
                    </div>

                    <div className="space-y-2">
                      <label className="text-sm font-bold text-slate-600 dark:text-slate-400">{t('adminUsers.specificBookPermission')}</label>
                      <div className="relative">
                        <input
                          type="text"
                          value={bookSearchQuery}
                          onChange={(e) => setBookSearchQuery(e.target.value)}
                          placeholder={t('adminUsers.searchBookPlaceholder')}
                          className="w-full pl-4 pr-10 py-2 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white text-sm"
                        />
                        {isSearchingBooks && (
                          <div className="absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 border-2 border-slate-300 border-t-primary-500 rounded-full animate-spin"></div>
                        )}
                        {(bookSearchResults.length > 0 || seriesSearchResults.length > 0) && (
                          <div className="absolute z-10 w-full mt-1 bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-xl shadow-xl max-h-64 overflow-y-auto">
                            {seriesSearchResults.length > 0 && (
                              <div className="py-1">
                                <div className="px-4 py-1 text-xs font-bold text-slate-400 uppercase tracking-wider bg-slate-50 dark:bg-slate-800/50">
                                  {t('adminUsers.series')}
                                </div>
                                {seriesSearchResults.map(series => (
                                  <button
                                    key={series.id}
                                    type="button"
                                    onClick={() => {
                                      const seriesBooks = series.books || [];
                                      const newBooks = seriesBooks.filter(sb => !selectedBooks.find(b => b.id === sb.id));
                                      
                                      if (newBooks.length > 0) {
                                        setSelectedBooks([...selectedBooks, ...newBooks]);
                                        setFormData(prev => ({
                                          ...prev,
                                          books_accessible: [...(prev.books_accessible || []), ...newBooks.map(b => b.id)]
                                        }));
                                      }
                                      setBookSearchQuery('');
                                      setBookSearchResults([]);
                                      setSeriesSearchResults([]);
                                    }}
                                    className="w-full text-left px-4 py-2 hover:bg-slate-50 dark:hover:bg-slate-800 text-sm dark:text-white flex items-center justify-between group"
                                  >
                                    <div className="flex flex-col truncate">
                                      <span className="truncate font-medium">{series.title}</span>
                                      <span className="text-xs text-slate-500 truncate">{t('adminUsers.seriesBookCount', { count: series.books?.length || 0 })}</span>
                                    </div>
                                    <Plus size={14} className="opacity-0 group-hover:opacity-100 text-primary-600 flex-shrink-0 ml-2" />
                                  </button>
                                ))}
                              </div>
                            )}

                            {bookSearchResults.length > 0 && (
                              <div className="py-1">
                                <div className="px-4 py-1 text-xs font-bold text-slate-400 uppercase tracking-wider bg-slate-50 dark:bg-slate-800/50">
                                  {t('adminUsers.books')}
                                </div>
                                {bookSearchResults.map(book => (
                                  <button
                                    key={book.id}
                                    type="button"
                                    onClick={() => {
                                      if (!selectedBooks.find(b => b.id === book.id)) {
                                        setSelectedBooks([...selectedBooks, book]);
                                        setFormData(prev => ({
                                          ...prev,
                                          books_accessible: [...(prev.books_accessible || []), book.id]
                                        }));
                                      }
                                      setBookSearchQuery('');
                                      setBookSearchResults([]);
                                      setSeriesSearchResults([]);
                                    }}
                                    className="w-full text-left px-4 py-2 hover:bg-slate-50 dark:hover:bg-slate-800 text-sm dark:text-white flex items-center justify-between group"
                                  >
                                    <span className="truncate">{book.title}</span>
                                    <Plus size={14} className="opacity-0 group-hover:opacity-100 text-primary-600 flex-shrink-0 ml-2" />
                                  </button>
                                ))}
                              </div>
                            )}
                          </div>
                        )}
                      </div>

                      <div className="flex flex-wrap gap-2 mt-2">
                        {selectedBooks.map(book => (
                          <div key={book.id} className="flex items-center gap-1 pl-2 pr-1 py-1 bg-primary-50 dark:bg-primary-900/20 text-primary-700 dark:text-primary-300 rounded-lg text-xs font-medium border border-primary-100 dark:border-primary-900/30">
                            <span className="max-w-[150px] truncate">{book.title}</span>
                            <button
                              type="button"
                              onClick={() => {
                                setSelectedBooks(selectedBooks.filter(b => b.id !== book.id));
                                setFormData(prev => ({
                                  ...prev,
                                  books_accessible: (prev.books_accessible || []).filter((id: string) => id !== book.id)
                                }));
                              }}
                              className="p-0.5 hover:bg-primary-100 dark:hover:bg-primary-900/40 rounded text-primary-500"
                            >
                              <X size={12} />
                            </button>
                          </div>
                        ))}
                      </div>
                      <p className="text-[10px] text-slate-400">
                        {t('adminUsers.permissionHint')}
                      </p>
                    </div>
                  </>
                )}

                <button 
                  type="submit"
                  className="w-full py-4 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-xl shadow-lg shadow-primary-500/30 transition-all mt-4"
                >
                  {editingId ? t('adminUsers.saveChanges') : t('adminUsers.createNow')}
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
