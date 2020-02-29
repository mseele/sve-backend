package de.sve.backend.manager;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import de.sve.backend.mail.Mail;
import de.sve.backend.model.contact.Message;

public class ContactManager {

	private static final Logger LOG = LoggerFactory.getLogger(ContactManager.class);

	public static void message(Message message) throws Exception {
		Mail.Builder mail = Mail.via(message.type().mailAccount());
		// subject
		mail.subject("[Kontakt@Web] Nachricht von " + message.name()); //$NON-NLS-1$
		// content
		StringBuilder content = new StringBuilder();
		content.append("\nVor- und Nachname: "); //$NON-NLS-1$
		content.append(message.name().strip());
		String email = message.email().strip();
		content.append("\nEmail: "); //$NON-NLS-1$
		content.append(email);
		String phone = message.phone();
		if (phone != null && phone.strip().length() > 0) {
			content.append("\nTelefon: "); //$NON-NLS-1$
			content.append(phone.strip());
		}
		content.append("\nNachricht:\n"); //$NON-NLS-1$
		content.append(message.message().strip());
		mail.content(content.toString())
			.to(message.to())
			.replyTo(email)
			.send();
		LOG.info("Info message has been send successfully"); //$NON-NLS-1$
	}

}
