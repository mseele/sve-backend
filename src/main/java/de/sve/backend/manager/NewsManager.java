package de.sve.backend.manager;

import de.sve.backend.model.news.Subscription;
import de.sve.backend.store.DataStore;

public class NewsManager {

	public static void subscribe(Subscription subscription) throws Exception {
		subscribe(subscription, true);
	}

	public static void subscribe(Subscription subscription, boolean sendMail) throws Exception {
		DataStore.subscribe(subscription);
		if (sendMail) {
			sendMail(subscription);
		}
	}

	public static void unsubscribe(Subscription subscription) throws Exception {
		DataStore.unsubscribe(subscription);
	}
	
	private static void sendMail(Subscription subscription) {
		// FIXME
//		try {
//			String subject;
//			
//			= "[Events@SVE] Bestätigung Event-Infos"; //$NON-NLS-1$
//
//			StringBuilder bodyBuilder = new StringBuilder("Hallo,\n" + //$NON-NLS-1$
//					"\n" + //$NON-NLS-1$
//					"vielen Dank für Dein Interesse an unseren Events.\n" + //$NON-NLS-1$
//					"\n" + //$NON-NLS-1$
//					"Ab sofort erhältst Du automatisch eine E-Mail, sobald neue Events online sind.\n" + //$NON-NLS-1$
//					"\n" + //$NON-NLS-1$
//					"Solltest Du an unserem E-Mail-Service kein Interesse mehr haben, kannst Du dich ganz einfach von diesem Angebot abmelden. \n" + //$NON-NLS-1$
//					"Klicke hierzu einfach auf folgenden Link:\n"); //$NON-NLS-1$
//			bodyBuilder.append(Utils.urlBuilder()
//								    .withPath("/unsubscribe") //$NON-NLS-1$
//								    .addParameter("email", email) //$NON-NLS-1$
//								    .toString());
//			bodyBuilder.append("\n" + //$NON-NLS-1$
//					"\n" + //$NON-NLS-1$
//					"Herzliche Grüße\n" + //$NON-NLS-1$
//					"Team Events@SVE"); //$NON-NLS-1$
//
//			send(email, subject, bodyBuilder.toString());
//			LOG.info("Subscription email was send successfully"); //$NON-NLS-1$
//		} catch (Throwable e) {
//			LOG.log(Level.SEVERE, "Error while sending subscription mail.", e); //$NON-NLS-1$
//			throw e;
//		}
	}

}
