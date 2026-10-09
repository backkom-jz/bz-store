export interface ApiEnvelope<T>{code:string;message:string;data:T;request_id:string}
export interface Page<T>{items:T[];total:number;page:number;page_size:number}
export interface PublicUser{id:string;email:string;display_name:string;is_admin:boolean}
export type ProductStatus='draft'|'published'|'archived';
export interface ProductSummary{id:string;title:string;slug:string;type:'tutorial'|'digital'|'service';description:string;price_cents:number;currency:'CNY';status:ProductStatus;category_id:string|null;is_demo:boolean;created_at:string}
export interface ProductDetail extends ProductSummary{tutorial_slug:string|null}
export interface Category{id:string;name:string;slug:string}
export interface ChapterSummary{id:string;title:string;sort_order:number;is_preview:boolean;status?:'draft'|'published';content_md?:string}
export interface TutorialDetail{id:string;title:string;slug:string;description:string;product_id:string;product_slug:string;product_status:ProductStatus;price_cents:number;is_demo:boolean;status:'draft'|'published';chapters:ChapterSummary[]}
export interface TutorialSummary extends Omit<TutorialDetail,'chapters'>{}
export type OrderStatus='pending_payment'|'payment_review'|'paid'|'fulfilling'|'completed'|'cancelled'|'refunded';
export interface PaymentSummary{id:string;order_id?:string;status:'pending'|'approved'|'rejected';review_note:string|null;created_at:string;reviewed_at:string|null}
export interface OrderItem{id:string;product_id:string;product_title:string;product_type:string;quantity:number;unit_price_cents:number;subtotal_cents:number}
export interface OrderDetail{id:string;order_no:string;user_id:string;amount_cents:number;currency:'CNY';status:OrderStatus;expires_at:string;paid_at:string|null;created_at:string;item:OrderItem;payments:PaymentSummary[]}
export interface PaymentInstructions{channel:'wechat'|'alipay'|'bank';payee:string;account:string;instructions:string;order_no:string;amount_cents:number;status:OrderStatus;expires_at:string}
export interface ChapterContent{id:string;title:string;content_md:string}
export interface LibraryEntry{product_id:string;title:string;tutorial_id:string;slug:string;is_demo:boolean;granted_at:string}
export interface AdminPayment extends PaymentSummary{order_id:string;order_no:string;amount_cents:number;payment_channel_expected:string;product_title:string;user_email:string;overdue:boolean;transaction_reference:string|null;actual_amount_cents:number|null;payment_channel:string|null}
export interface AuditLog{id:string;actor_email:string|null;action:string;target_id:string;detail:Record<string,unknown>;created_at:string}
export interface ApproveInput{actual_amount_cents:number;payment_channel:string;transaction_reference:string;receipt_confirmed:boolean}
