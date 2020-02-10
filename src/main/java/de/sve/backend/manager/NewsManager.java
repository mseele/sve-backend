package de.sve.backend.manager;

import java.util.stream.Collectors;

import de.sve.backend.Utils;
import de.sve.backend.mail.Mail;
import de.sve.backend.mail.MailAccount;
import de.sve.backend.model.news.NewsType;
import de.sve.backend.model.news.Subscription;
import de.sve.backend.store.DataStore;

public class NewsManager {
	
	private static final String UNSUBSCRIBE_URL = Utils.urlBuilder()
		    										   .withPath("/newsletter#abmelden") //$NON-NLS-1$
		    										   .toString();

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

	private static void sendMail(Subscription subscription) throws Exception {
		if (subscription.types().size() == 1) {
			NewsType type = subscription.types().iterator().next();
			MailAccount mailAccount = type.mailAccount();
			Mail.Builder builder = Mail.via(mailAccount)
									   .to(subscription.email())
									   .bcc(mailAccount.email());
			switch (type) {
				case Events:
					builder.subject("[Events@SVE] Bestätigung Event-News Anmeldung") //$NON-NLS-1$
						   .content("Hallo,\n" + //$NON-NLS-1$
									"\n" + //$NON-NLS-1$
									"vielen Dank für Dein Interesse an unseren Events.\n" + //$NON-NLS-1$
									"\n" + //$NON-NLS-1$
									"Ab sofort erhältst Du automatisch eine E-Mail, sobald neue Events online sind.\n" + //$NON-NLS-1$
									"\n" + //$NON-NLS-1$
									"Solltest Du an unserem E-Mail-Service kein Interesse mehr haben, kannst Du dich hier wieder abmelden:\n" + //$NON-NLS-1$
									UNSUBSCRIBE_URL + "\n" + //$NON-NLS-1$
									"\n" + //$NON-NLS-1$
									"Herzliche Grüße\n" + //$NON-NLS-1$
									"Team Events@SVE"); //$NON-NLS-1$
					break;
				case Fitness:
					builder.subject("[Fitness@SVE] Bestätigung Fitness-News Anmeldung") //$NON-NLS-1$
						   .content("Hallo,\n" + //$NON-NLS-1$
									"\n" + //$NON-NLS-1$
									"vielen Dank für Dein Interesse an unseren Fitnesskursen.\n" + //$NON-NLS-1$
									"\n" + //$NON-NLS-1$
									"Ab sofort erhältst Du automatisch eine E-Mail, sobald neue Kurse online sind.\n" + //$NON-NLS-1$
									"\n" + //$NON-NLS-1$
									"Solltest Du an unserem E-Mail-Service kein Interesse mehr haben, kannst Du dich hier wieder abmelden:\n" + //$NON-NLS-1$
									UNSUBSCRIBE_URL + "\n" + //$NON-NLS-1$
									"\n" + //$NON-NLS-1$
									"Herzliche Grüße\n" + //$NON-NLS-1$
									"Team Fitness@SVE"); //$NON-NLS-1$
					break;
				case General:
					builder.subject("[Infos@SVE] Bestätigung Newsletter Anmeldung") //$NON-NLS-1$
						   .content("Hallo,\n" + //$NON-NLS-1$
									"\n" + //$NON-NLS-1$
									"vielen Dank für Dein Interesse an News rund um den SVE.\n" + //$NON-NLS-1$
									"\n" + //$NON-NLS-1$
									"Ab sofort erhältst Du automatisch eine E-Mail, sobald es etwas neues gibt.\n" + //$NON-NLS-1$
									"\n" + //$NON-NLS-1$
									"Solltest Du an unserem E-Mail-Service kein Interesse mehr haben, kannst Du dich hier wieder abmelden:\n" + //$NON-NLS-1$
									UNSUBSCRIBE_URL + "\n" + //$NON-NLS-1$
									"\n" + //$NON-NLS-1$
									"Herzliche Grüße\n" + //$NON-NLS-1$
									"SV Eutingen"); //$NON-NLS-1$
					break;
				default:
					break;
			}
			if (!builder.send()) {
				throw new Exception("Error while sending email"); //$NON-NLS-1$
			}
		} else {
			String types = subscription.types()
									   .stream()
									   .map(NewsType::displayName)
									   .collect(Collectors.joining(", ")); //$NON-NLS-1$
			MailAccount mailAccount = MailAccount.INFO;
			if (!Mail.via(mailAccount)
					 .to(subscription.email())
					 .bcc(mailAccount.email())
					 .subject("[Infos@SVE] Bestätigung Newsletter Anmeldung") //$NON-NLS-1$
					 .content("Hallo,\n" + //$NON-NLS-1$
							  "\n" + //$NON-NLS-1$
							  "vielen Dank für Dein Interesse an News rund um den SVE.\n" + //$NON-NLS-1$
							  "\n" + //$NON-NLS-1$
							  "Ab sofort erhältst Du automatisch eine E-Mail zu folgenden Themen: " + types + ".\n" + //$NON-NLS-1$ //$NON-NLS-2$
							  "\n" + //$NON-NLS-1$
							  "Solltest Du an unserem E-Mail-Service kein Interesse mehr haben, kannst Du dich hier abmelden:\n" + //$NON-NLS-1$
							  UNSUBSCRIBE_URL + "\n" + //$NON-NLS-1$
							  "\n" + //$NON-NLS-1$
							  "Herzliche Grüße\n" + //$NON-NLS-1$
							 "SV Eutingen") //$NON-NLS-1$
					 .send()) {
				throw new Exception("Error while sending email"); //$NON-NLS-1$
			}			
		}
	}

}
